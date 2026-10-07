package dev.gpui.android

import android.graphics.SurfaceTexture
import android.opengl.EGL14
import android.opengl.EGLConfig
import android.opengl.GLES11Ext
import android.opengl.GLES20
import android.os.Handler
import android.view.Surface
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.ArrayDeque

/** Converts decoder surfaces into top-to-bottom RGBA frames on a dedicated GL thread. */
internal class MediaFrames(
    private val handler: Handler,
    private val session: Long,
    private val nativeFrames: Boolean,
    private val acceptsFrame: (Long) -> Boolean,
    private val publish: (ByteBuffer, Int, Int, Long, Long) -> Unit,
) : AutoCloseable {
    var width = 0
    var height = 0
    var onError: ((Exception) -> Unit)? = null
    lateinit var surface: Surface
        private set
    private var display = EGL14.EGL_NO_DISPLAY
    private var context = EGL14.EGL_NO_CONTEXT
    private var pbuffer = EGL14.EGL_NO_SURFACE
    private var texture: SurfaceTexture? = null
    private var external = 0
    private var target = 0
    private var framebuffer = 0
    private var program = 0
    private var maxSize = 0
    private var allocatedWidth = 0
    private var allocatedHeight = 0
    private var pixels: ByteBuffer? = null
    private var nativePool = 0L
    private val matrix = FloatArray(16)
    private data class Timestamp(val release: Long, val pts: Long, val generation: Long, val width: Int, val height: Int)
    private var lastStamp: Timestamp? = null
    private val timestamps = ArrayDeque<Timestamp>()
    private val vertices = ByteBuffer.allocateDirect(16 * 4).order(ByteOrder.nativeOrder()).asFloatBuffer().apply {
        put(floatArrayOf(-1f, -1f, 0f, 1f, 1f, -1f, 1f, 1f, -1f, 1f, 0f, 0f, 1f, 1f, 1f, 0f))
        position(0)
    }

    fun initialize() {
        display = EGL14.eglGetDisplay(EGL14.EGL_DEFAULT_DISPLAY)
        check(display != EGL14.EGL_NO_DISPLAY)
        val version = IntArray(2)
        check(EGL14.eglInitialize(display, version, 0, version, 1))
        val configs = arrayOfNulls<EGLConfig>(1)
        val count = IntArray(1)
        check(EGL14.eglChooseConfig(display, intArrayOf(
            EGL14.EGL_RENDERABLE_TYPE, EGL14.EGL_OPENGL_ES2_BIT,
            EGL14.EGL_SURFACE_TYPE, EGL14.EGL_PBUFFER_BIT,
            EGL14.EGL_RED_SIZE, 8, EGL14.EGL_GREEN_SIZE, 8, EGL14.EGL_BLUE_SIZE, 8, EGL14.EGL_ALPHA_SIZE, 8,
            EGL14.EGL_NONE), 0, configs, 0, 1, count, 0) && count[0] > 0)
        context = EGL14.eglCreateContext(display, configs[0], EGL14.EGL_NO_CONTEXT,
            intArrayOf(EGL14.EGL_CONTEXT_CLIENT_VERSION, 2, EGL14.EGL_NONE), 0)
        check(context != EGL14.EGL_NO_CONTEXT)
        pbuffer = EGL14.eglCreatePbufferSurface(display, configs[0], intArrayOf(EGL14.EGL_WIDTH, 1, EGL14.EGL_HEIGHT, 1, EGL14.EGL_NONE), 0)
        check(pbuffer != EGL14.EGL_NO_SURFACE)
        check(EGL14.eglMakeCurrent(display, pbuffer, pbuffer, context))
        val maximum = IntArray(1)
        GLES20.glGetIntegerv(GLES20.GL_MAX_TEXTURE_SIZE, maximum, 0)
        maxSize = maximum[0]
        val names = IntArray(2)
        GLES20.glGenTextures(2, names, 0)
        external = names[0]
        target = names[1]
        GLES20.glBindTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, external)
        textureParameters(GLES11Ext.GL_TEXTURE_EXTERNAL_OES)
        val fbo = IntArray(1)
        GLES20.glGenFramebuffers(1, fbo, 0)
        framebuffer = fbo[0]
        program = createProgram()
        texture = SurfaceTexture(external).also { source ->
            surface = Surface(source)
            source.setOnFrameAvailableListener({
                try { capture(source) } catch (error: Exception) { onError?.invoke(error) }
            }, handler)
        }
        checkGl()
        if (nativeFrames) nativePool = nativeCreate()
    }

    @Synchronized fun recordTimestamp(release: Long, pts: Long, generation: Long, width: Int, height: Int) {
        while (timestamps.size >= 16) timestamps.removeFirst()
        timestamps.addLast(Timestamp(release, pts, generation, width, height))
    }

    @Synchronized fun discardPending() { timestamps.clear(); lastStamp = null }

    @Synchronized private fun timestamp(release: Long): Timestamp? {
        var selected: Timestamp? = null
        while (timestamps.isNotEmpty() && timestamps.first.release <= release) selected = timestamps.removeFirst()
        return selected
    }

    fun useCpuFrames() {
        if (nativePool != 0L) nativeClose(nativePool)
        nativePool = 0L
        texture?.let { capture(it, true) }
    }

    private fun capture(source: SurfaceTexture, refresh: Boolean = false) {
        if (!refresh) source.updateTexImage()
        val stamp = (if (refresh) lastStamp else timestamp(source.timestamp)) ?: return
        if (!acceptsFrame(stamp.generation)) return
        lastStamp = stamp
        if (stamp.width > 0 && stamp.height > 0) {
            width = stamp.width
            height = stamp.height
        }
        if (width <= 0 || height <= 0) return
        check(width <= maxSize && height <= maxSize) { "Video exceeds GL texture limits" }
        var gpuTarget = if (nativePool != 0L) nativeTarget(nativePool, width, height) else -1
        if (gpuTarget == 0) return // All bounded pool slots are still in use.
        if (gpuTarget < 0 && nativePool != 0L) {
            nativeClose(nativePool)
            nativePool = 0L
        }
        if (gpuTarget > 0) {
            GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, framebuffer)
            GLES20.glFramebufferTexture2D(GLES20.GL_FRAMEBUFFER, GLES20.GL_COLOR_ATTACHMENT0, GLES20.GL_TEXTURE_2D, gpuTarget, 0)
            if (GLES20.glCheckFramebufferStatus(GLES20.GL_FRAMEBUFFER) != GLES20.GL_FRAMEBUFFER_COMPLETE) {
                nativeClose(nativePool)
                nativePool = 0L
                gpuTarget = -1
            }
        }
        if (gpuTarget < 0 && (width != allocatedWidth || height != allocatedHeight)) {
            val byteCount = Math.multiplyExact(Math.multiplyExact(width, height), 4)
            pixels = ByteBuffer.allocateDirect(byteCount).order(ByteOrder.nativeOrder())
            GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, target)
            textureParameters(GLES20.GL_TEXTURE_2D)
            GLES20.glTexImage2D(GLES20.GL_TEXTURE_2D, 0, GLES20.GL_RGBA, width, height, 0, GLES20.GL_RGBA, GLES20.GL_UNSIGNED_BYTE, null)
            GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, framebuffer)
            GLES20.glFramebufferTexture2D(GLES20.GL_FRAMEBUFFER, GLES20.GL_COLOR_ATTACHMENT0, GLES20.GL_TEXTURE_2D, target, 0)
            check(GLES20.glCheckFramebufferStatus(GLES20.GL_FRAMEBUFFER) == GLES20.GL_FRAMEBUFFER_COMPLETE)
            allocatedWidth = width
            allocatedHeight = height
        }
        GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, framebuffer)
        if (gpuTarget < 0) GLES20.glFramebufferTexture2D(GLES20.GL_FRAMEBUFFER, GLES20.GL_COLOR_ATTACHMENT0, GLES20.GL_TEXTURE_2D, target, 0)
        GLES20.glViewport(0, 0, width, height)
        GLES20.glUseProgram(program)
        source.getTransformMatrix(matrix)
        GLES20.glUniformMatrix4fv(GLES20.glGetUniformLocation(program, "transform"), 1, false, matrix, 0)
        GLES20.glActiveTexture(GLES20.GL_TEXTURE0)
        GLES20.glBindTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, external)
        GLES20.glUniform1i(GLES20.glGetUniformLocation(program, "image"), 0)
        val position = GLES20.glGetAttribLocation(program, "position")
        val uv = GLES20.glGetAttribLocation(program, "uv")
        vertices.position(0)
        GLES20.glVertexAttribPointer(position, 2, GLES20.GL_FLOAT, false, 16, vertices)
        vertices.position(2)
        GLES20.glVertexAttribPointer(uv, 2, GLES20.GL_FLOAT, false, 16, vertices)
        GLES20.glEnableVertexAttribArray(position)
        GLES20.glEnableVertexAttribArray(uv)
        GLES20.glDrawArrays(GLES20.GL_TRIANGLE_STRIP, 0, 4)
        checkGl()
        if (gpuTarget > 0) {
            if (nativePublish(nativePool, session, stamp.generation, stamp.pts) == 0) {
                useCpuFrames()
            }
            return
        }
        val buffer = requireNotNull(pixels).apply { clear() }
        GLES20.glReadPixels(0, 0, width, height, GLES20.GL_RGBA, GLES20.GL_UNSIGNED_BYTE, buffer)
        checkGl()
        publish(buffer, width, height, stamp.pts, stamp.generation)
    }

    private fun textureParameters(kind: Int) {
        GLES20.glTexParameteri(kind, GLES20.GL_TEXTURE_MIN_FILTER, GLES20.GL_LINEAR)
        GLES20.glTexParameteri(kind, GLES20.GL_TEXTURE_MAG_FILTER, GLES20.GL_LINEAR)
        GLES20.glTexParameteri(kind, GLES20.GL_TEXTURE_WRAP_S, GLES20.GL_CLAMP_TO_EDGE)
        GLES20.glTexParameteri(kind, GLES20.GL_TEXTURE_WRAP_T, GLES20.GL_CLAMP_TO_EDGE)
    }

    private fun createProgram(): Int {
        val vertex = shader(GLES20.GL_VERTEX_SHADER, """
            attribute vec2 position;
            attribute vec2 uv;
            uniform mat4 transform;
            varying vec2 coords;
            void main() {
                gl_Position = vec4(position, 0.0, 1.0);
                coords = (transform * vec4(uv, 0.0, 1.0)).xy;
            }
        """.trimIndent())
        var fragment = 0
        val result = GLES20.glCreateProgram()
        try {
            fragment = shader(GLES20.GL_FRAGMENT_SHADER, """
                #extension GL_OES_EGL_image_external : require
                precision mediump float;
                uniform samplerExternalOES image;
                varying vec2 coords;
                void main() { gl_FragColor = texture2D(image, coords); }
            """.trimIndent())
            GLES20.glAttachShader(result, vertex)
            GLES20.glAttachShader(result, fragment)
            GLES20.glLinkProgram(result)
            val status = IntArray(1)
            GLES20.glGetProgramiv(result, GLES20.GL_LINK_STATUS, status, 0)
            check(status[0] != 0) { GLES20.glGetProgramInfoLog(result) }
            return result
        } catch (error: Exception) {
            GLES20.glDeleteProgram(result)
            throw error
        } finally {
            GLES20.glDeleteShader(vertex)
            if (fragment != 0) GLES20.glDeleteShader(fragment)
        }
    }

    private fun shader(kind: Int, source: String): Int {
        val shader = GLES20.glCreateShader(kind)
        GLES20.glShaderSource(shader, source)
        GLES20.glCompileShader(shader)
        val status = IntArray(1)
        GLES20.glGetShaderiv(shader, GLES20.GL_COMPILE_STATUS, status, 0)
        if (status[0] == 0) {
            val message = GLES20.glGetShaderInfoLog(shader)
            GLES20.glDeleteShader(shader)
            error(message)
        }
        return shader
    }

    private fun checkGl() { check(GLES20.glGetError() == GLES20.GL_NO_ERROR) { "Video GL operation failed" } }

    private external fun nativeCreate(): Long
    private external fun nativeTarget(pool: Long, width: Int, height: Int): Int
    private external fun nativePublish(pool: Long, session: Long, generation: Long, timestamp: Long): Int
    private external fun nativeClose(pool: Long)

    override fun close() {
        texture?.setOnFrameAvailableListener(null)
        if (::surface.isInitialized) surface.release()
        texture?.release()
        texture = null
        if (context != EGL14.EGL_NO_CONTEXT) {
            EGL14.eglMakeCurrent(display, pbuffer, pbuffer, context)
            if (nativePool != 0L) nativeClose(nativePool)
            nativePool = 0L
            GLES20.glDeleteTextures(2, intArrayOf(external, target), 0)
            GLES20.glDeleteFramebuffers(1, intArrayOf(framebuffer), 0)
            GLES20.glDeleteProgram(program)
            EGL14.eglMakeCurrent(display, EGL14.EGL_NO_SURFACE, EGL14.EGL_NO_SURFACE, EGL14.EGL_NO_CONTEXT)
            EGL14.eglDestroyContext(display, context)
        }
        if (pbuffer != EGL14.EGL_NO_SURFACE) EGL14.eglDestroySurface(display, pbuffer)
        if (display != EGL14.EGL_NO_DISPLAY) EGL14.eglTerminate(display)
        EGL14.eglReleaseThread()
        pixels = null
        discardPending()
    }
}
