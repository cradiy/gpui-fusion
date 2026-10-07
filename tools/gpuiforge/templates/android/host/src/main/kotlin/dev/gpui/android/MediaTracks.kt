package dev.gpui.android

import androidx.media3.common.C
import androidx.media3.common.TrackGroup
import androidx.media3.common.Tracks
import androidx.media3.common.util.UnstableApi
import org.json.JSONArray
import org.json.JSONObject

@UnstableApi
internal class MediaTracks(private val session: Long, private val publish: (Long, String) -> Unit) {
    data class Choice(val group: TrackGroup, val index: Int, val type: Int)
    @Volatile private var choices: Map<String, Choice> = emptyMap()
    private val groups = mutableMapOf<TrackGroup, Long>()
    private var nextGroup = 0L
    private var lastJson = ""
    private var lastGeneration = -1L
    var selectedSubtitle: String? = null
        private set

    fun find(id: String): Choice? = choices[id]

    fun reset(generation: Long) {
        choices = emptyMap()
        groups.clear()
        lastJson = ""
        report(Tracks.EMPTY, generation)
    }

    fun report(tracks: Tracks, generation: Long) {
        val relevant = tracks.groups.filter { it.type == C.TRACK_TYPE_AUDIO || it.type == C.TRACK_TYPE_TEXT }
        groups.keys.retainAll(relevant.map { it.mediaTrackGroup }.toSet())
        val available = mutableMapOf<String, Choice>()
        val audio = JSONArray()
        val subtitles = JSONArray()
        selectedSubtitle = null
        for (group in relevant) {
            val key = groups.getOrPut(group.mediaTrackGroup) { nextGroup++ }
            for (index in 0 until group.length) {
                if (!group.isTrackSupported(index)) continue
                val format = group.getTrackFormat(index)
                val isText = group.type == C.TRACK_TYPE_TEXT
                if (isText && format.sampleMimeType !in textFormats && format.codecs !in textFormats) continue
                val id = "${if (isText) "text" else "audio"}:$session:$key:$index"
                available[id] = Choice(group.mediaTrackGroup, index, group.type)
                if (isText && group.isTrackSelected(index)) selectedSubtitle = id
                (if (isText) subtitles else audio).put(JSONObject().apply {
                    put("id", id)
                    put("codec", format.codecs ?: format.sampleMimeType)
                    put("language", format.language?.takeUnless { it.isBlank() || it == "und" })
                    put("title", format.label)
                    if (format.channelCount > 0) put("channels", format.channelCount)
                    if (format.sampleRate > 0) put("sample_rate", format.sampleRate)
                    if (format.bitrate > 0) put("bitrate", format.bitrate)
                    put("selected", group.isTrackSelected(index))
                    put("forced", format.selectionFlags and C.SELECTION_FLAG_FORCED != 0)
                })
            }
        }
        choices = available
        val encoded = JSONObject().put("audio", audio).put("subtitles", subtitles).toString()
        if (encoded != lastJson || generation != lastGeneration) {
            lastJson = encoded
            lastGeneration = generation
            publish(generation, encoded)
        }
    }

    private companion object {
        val textFormats = setOf("application/x-subrip", "text/vtt", "text/x-ssa",
            "application/ttml+xml", "application/x-quicktime-tx3g")
    }
}
