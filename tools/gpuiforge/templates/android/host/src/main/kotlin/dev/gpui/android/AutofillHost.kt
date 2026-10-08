package dev.gpui.android

import android.graphics.Rect
import android.text.InputType
import android.util.SparseArray
import android.view.View
import android.view.ViewStructure
import android.view.autofill.AutofillManager
import android.view.autofill.AutofillValue
import org.json.JSONArray
import kotlin.math.ceil
import kotlin.math.floor

internal class AutofillHost(private val view: GpuiView, private val session: GpuiSession) {
    private class Field(val key: String, val id: Int, val name: String, val hint: String, val value: String, val bounds: Rect, val focused: Boolean)
    private val manager get() = view.context.getSystemService(AutofillManager::class.java)
    private val ids = mutableMapOf<Triple<String, String, String>, Int>()
    private var nextId = 1
    private var fields = emptyList<Field>()
    private var previous = emptyList<Field>()
    private var entered: Field? = null
    private var finishedField: Int? = null

    fun canRequest(): Boolean = fields.any { it.focused } && manager?.isEnabled == true

    fun request(): Boolean {
        val field = fields.firstOrNull { it.focused } ?: return false
        val afm = manager?.takeIf { it.isEnabled } ?: return false
        if (!view.hasWindowFocus()) return false
        finishedField = null
        sync()
        afm.requestAutofill(view, field.id, screenBounds(field))
        return true
    }

    private fun screenBounds(field: Field): Rect {
        val location = IntArray(2)
        view.getLocationOnScreen(location)
        return Rect(field.bounds).apply { offset(location[0], location[1]) }
    }

    fun update(json: String) {
        val array = JSONArray(json)
        fields = (0 until array.length()).map { index ->
            val item = array.getJSONObject(index)
            val key = item.getString("id")
            val name = item.getString("name")
            val hint = item.getString("hint")
            val x = item.getDouble("x")
            val y = item.getDouble("y")
            Field(key, ids.getOrPut(Triple(key, name, hint)) { nextId++ }, name, hint, item.getString("value"),
                Rect(floor(x).toInt(), floor(y).toInt(), ceil(x + item.getDouble("width")).toInt(), ceil(y + item.getDouble("height")).toInt()),
                item.getBoolean("focused"))
        }
        // Never reuse virtual IDs, even when a field disappears and later returns.
        ids.keys.retainAll(fields.map { Triple(it.key, it.name, it.hint) }.toSet())
    }

    fun sync() {
        val afm = manager ?: return
        val focused = fields.firstOrNull { it.focused }
        if (focused?.id != finishedField) finishedField = null
        val next = focused?.takeIf { it.id != finishedField && view.hasWindowFocus() }
        if (entered?.id != next?.id || entered?.bounds != next?.bounds) {
            entered?.let { afm.notifyViewExited(view, it.id) }
            entered = next
            next?.let {
                afm.notifyViewEntered(view, it.id, screenBounds(it))
            }
        }
        for (field in fields) {
            val old = previous.firstOrNull { it.id == field.id }
            if (old != null && old.value != field.value) {
                afm.notifyValueChanged(view, field.id, AutofillValue.forText(field.value))
            }
        }
        previous = fields
    }

    fun structure(root: ViewStructure) {
        val parent = root.autofillId ?: return
        val start = root.addChildCount(fields.size)
        for ((index, field) in fields.withIndex()) {
            root.newChild(start + index).apply {
                setAutofillId(parent, field.id)
                setId(field.id, view.context.packageName, "id", field.name)
                setClassName("android.widget.EditText")
                setAutofillType(View.AUTOFILL_TYPE_TEXT)
                setAutofillHints(arrayOf(field.hint))
                setAutofillValue(AutofillValue.forText(field.value))
                setText(field.value)
                setDataIsSensitive(true)
                setFocusable(true)
                setFocused(field.focused)
                setEnabled(true)
                setVisibility(View.VISIBLE)
                setInputType(when (field.hint) {
                    "password", "newPassword" -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
                    "emailAddress" -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS
                    "phoneNumber" -> InputType.TYPE_CLASS_PHONE
                    else -> InputType.TYPE_CLASS_TEXT
                })
                setDimens(field.bounds.left, field.bounds.top, 0, 0, field.bounds.width(), field.bounds.height())
            }
        }
    }

    fun fill(values: SparseArray<AutofillValue>) {
        for (index in 0 until values.size()) {
            val value = values.valueAt(index)
            val field = fields.firstOrNull { it.id == values.keyAt(index) } ?: continue
            if (value.isText) session.autofill(field.key, value.textValue.toString())
        }
    }

    fun finish(commit: Boolean) {
        sync()
        if (commit) manager?.commit() else manager?.cancel()
        finishedField = fields.firstOrNull { it.focused }?.id
        entered = null
    }

    fun detach() {
        entered?.let { manager?.notifyViewExited(view, it.id) }
        entered = null
        fields = emptyList()
        previous = emptyList()
        ids.clear()
    }
}
