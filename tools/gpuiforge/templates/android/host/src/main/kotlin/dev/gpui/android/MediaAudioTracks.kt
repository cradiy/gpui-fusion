package dev.gpui.android

import androidx.media3.common.C
import androidx.media3.common.TrackGroup
import androidx.media3.common.Tracks
import androidx.media3.common.util.UnstableApi
import org.json.JSONArray
import org.json.JSONObject

@UnstableApi
internal class MediaAudioTracks(private val session: Long, private val publish: (Long, String) -> Unit) {
    data class Choice(val group: TrackGroup, val index: Int)
    @Volatile private var choices: Map<String, Choice> = emptyMap()
    private val groups = mutableMapOf<TrackGroup, Long>()
    private var nextGroup = 0L
    private var lastJson = ""
    private var lastGeneration = -1L

    fun find(id: String): Choice? = choices[id]

    fun reset(generation: Long) {
        choices = emptyMap()
        groups.clear()
        lastJson = ""
        report(Tracks.EMPTY, generation)
    }

    fun report(tracks: Tracks, generation: Long) {
        val audio = tracks.groups.filter { it.type == C.TRACK_TYPE_AUDIO }
        groups.keys.retainAll(audio.map { it.mediaTrackGroup }.toSet())
        val available = mutableMapOf<String, Choice>()
        val json = JSONArray()
        for (group in audio) {
            val key = groups.getOrPut(group.mediaTrackGroup) { nextGroup++ }
            for (index in 0 until group.length) {
                if (!group.isTrackSupported(index)) continue
                val format = group.getTrackFormat(index)
                val id = "audio:$session:$key:$index"
                available[id] = Choice(group.mediaTrackGroup, index)
                json.put(JSONObject().apply {
                    put("id", id)
                    put("codec", format.codecs ?: format.sampleMimeType)
                    put("language", format.language?.takeUnless { it.isBlank() || it == "und" })
                    put("title", format.label)
                    if (format.channelCount > 0) put("channels", format.channelCount)
                    if (format.sampleRate > 0) put("sample_rate", format.sampleRate)
                    if (format.bitrate > 0) put("bitrate", format.bitrate)
                    put("selected", group.isTrackSelected(index))
                })
            }
        }
        choices = available
        val encoded = json.toString()
        if (encoded != lastJson || generation != lastGeneration) {
            lastJson = encoded
            lastGeneration = generation
            publish(generation, encoded)
        }
    }
}
