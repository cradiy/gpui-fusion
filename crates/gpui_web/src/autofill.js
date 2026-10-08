import { configureImeInput, positionImeInput } from "./ime.js";

export class WebAutofill {
    constructor(canvas, ime, fill, focus) {
        this.canvas = canvas;
        this.ime = ime;
        this.fill = fill;
        this.focusField = focus;
        this.fields = new Map();
        this.active = ime;
        this.dismissedField = null;
        this.composing = false;
        this.disposed = false;
        this.form = document.createElement("form");
        this.form.id = `${canvas.id || "gpui"}-autofill`;
        this.form.style.cssText = "position: relative; isolation: isolate; width: 100%; height: 100%; margin: 0; touch-action: none;";
        this.form.autocomplete = "on";
        this.form.method = "post";
        this.form.noValidate = true;
        this.form.addEventListener("submit", e => e.preventDefault());
        this.styles = document.createElement("style");
        this.styles.textContent = `.gpui-autofill-input::selection { color: transparent; background: transparent; }`;
        this.form.append(this.styles);
        ime.autocomplete = "off";
        ime.setAttribute("aria-hidden", "true");
        const wasFocused = document.activeElement === ime;
        ime.before(this.form);
        // The canvas paints above the native controls, including the browser's
        // autofill highlight. Hit testing still reaches the real form inputs.
        canvas.style.position = "relative";
        canvas.style.zIndex = "1";
        canvas.style.pointerEvents = "none";
        this.form.append(canvas, ime);
        this.relayPointerEvents();
        this.form.addEventListener("focusin", e => {
            const entry = this.fields.get(e.target.dataset?.gpuiField);
            if (!entry) return;
            this.dismissedField = null;
            this.active = entry.input;
            if (entry.focused) return;
            queueMicrotask(() => {
                if (!this.disposed && this.fields.get(entry.id) === entry && document.activeElement === entry.input) {
                    this.focusField(entry.id);
                }
            });
        });
        if (wasFocused) ime.focus({ preventScroll: true });
        this.form.addEventListener("compositionstart", () => { this.composing = true; });
        this.form.addEventListener("compositionend", () => {
            // The final input event follows compositionend in some engines.
            queueMicrotask(() => { this.composing = false; });
        });
        for (const type of ["input", "change"]) this.form.addEventListener(type, e => {
            if (this.composing || e.isComposing || (e.inputType || "").includes("Composition")) return;
            const entry = this.fields.get(e.target.dataset.gpuiField);
            if (entry) this.changed(entry);
        });
    }

    update(json, width, height) {
        const ownedFocus = this.form.contains(document.activeElement);
        const fields = JSON.parse(json);
        const live = new Set(fields.map(field => field.id));
        for (const [id, entry] of this.fields) {
            if (!live.has(id)) {
                entry.input.value = "";
                entry.input.remove();
                this.fields.delete(id);
            }
        }
        let active = this.ime;
        for (const field of fields) {
            let entry = this.fields.get(field.id);
            if (entry && (entry.input.name !== field.name || entry.input.autocomplete !== field.hint)) {
                entry.input.value = "";
                entry.input.remove();
                this.fields.delete(field.id);
                entry = null;
            }
            if (!entry) {
                const input = document.createElement("input");
                configureImeInput(input);
                // Keep the control hit-testable so password managers receive the
                // user's real click. GPUI supplies its text, caret and background.
                input.className = "gpui-autofill-input";
                Object.assign(input.style, {
                    opacity: "1", pointerEvents: "auto", color: "transparent",
                    caretColor: "transparent", background: "transparent",
                    webkitTextFillColor: "transparent", cursor: "text",
                });
                input.tabIndex = 0;
                input.dataset.gpuiField = field.id;
                input.name = field.name;
                input.setAttribute("aria-label", field.name);
                input.id = `${this.canvas.id || "gpui"}-${field.name}`;
                input.autocomplete = field.hint;
                input.type = field.hint.includes("password") ? "password" : "text";
                this.form.append(input);
                input.value = field.value;
                entry = { id: field.id, input, value: field.value, pending: null };
                this.fields.set(field.id, entry);
            }
            const input = entry.input;
            entry.focused = field.focused;
            // Preserve a browser fill until Rust has applied its callback.
            if (!this.composing) {
                if (entry.pending === null && input.value !== entry.value) this.changed(entry);
                if (entry.pending === null || field.value !== entry.value || field.value === entry.pending) {
                    const edited = input.value !== field.value;
                    // Even assigning the same value clears Chrome's pending
                    // autofill preview before it is revealed by a user gesture.
                    if (edited) input.value = field.value;
                    entry.value = field.value;
                    entry.pending = null;
                    // GPUI edits are controlled values. Notify form observers
                    // after paint without recursively applying the same edit.
                    if (edited) queueMicrotask(() => {
                        if (!this.disposed && this.fields.get(entry.id) === entry) {
                            input.dispatchEvent(new Event("input", { bubbles: true }));
                        }
                    });
                }
            }
            positionImeInput(this.canvas, input, field.x, field.y, field.height, width, height, field.width);
            if (field.focused) {
                if (this.dismissedField !== field.id) {
                    this.dismissedField = null;
                    active = input;
                }
            }
        }
        if (active !== this.active) {
            this.active = active;
            // Wait until the GPUI draw releases App before DOM focus callbacks run.
            queueMicrotask(() => {
                if (ownedFocus && this.active === active && active.isConnected) active.focus({ preventScroll: true });
            });
        }
    }

    relayPointerEvents() {
        for (const type of ["pointerdown", "pointermove", "pointerup", "pointercancel", "pointerenter", "pointerleave", "contextmenu", "wheel", "dragover", "drop", "dragleave"]) {
            this.form.addEventListener(type, e => {
                const entry = this.fields.get(e.target.dataset?.gpuiField);
                if (e.target !== this.form && entry?.input !== e.target) return;
                if (type === "pointerdown") {
                    if (entry) {
                        this.dismissedField = null;
                        this.active = entry.input;
                    } else {
                        this.dismissedField = this.active.dataset.gpuiField || this.dismissedField;
                        this.active = this.ime;
                        e.preventDefault();
                    }
                }
                const EventType = type === "wheel" ? WheelEvent : type === "contextmenu" ? MouseEvent
                    : type.startsWith("drag") || type === "drop" ? DragEvent : PointerEvent;
                const forwarded = new EventType(type, e);
                this.canvas.dispatchEvent(forwarded);
                // The original pointerdown must retain its native focus/click
                // behavior; cancelling the forwarded GPUI event is independent.
                if ((type === "wheel" || type === "contextmenu" || type.startsWith("drag") || type === "drop") && forwarded.defaultPrevented) e.preventDefault();
            }, { passive: false });
        }
    }

    listen(name, handler) {
        const listener = e => {
            if (this.disposed) return;
            if (name === "focus" || name === "blur") {
                if (this.form.contains(e.relatedTarget)) return;
                queueMicrotask(() => { if (!this.disposed) handler(e); });
            } else handler(e);
        };
        this.form.addEventListener(name, listener, true);
    }

    changed(entry) {
        if (entry.value !== entry.input.value && entry.pending !== entry.input.value) {
            entry.pending = entry.input.value;
            // A password manager can fill during a GPUI draw; avoid reentering App.
            const value = entry.pending;
            queueMicrotask(() => {
                if (this.fields.get(entry.id) === entry && entry.pending === value) this.fill(entry.id, value);
            });
        }
    }

    focus() { this.active.focus({ preventScroll: true }); }

    selection(start, end) {
        if (this.active !== this.ime && !this.composing) this.active.setSelectionRange(start, end);
    }

    finish(commit) {
        // Browsers decide whether a successfully completed form qualifies for
        // saving. Never send credentials as a network form submission.
        if (commit) {
            this.form.requestSubmit();
            if (window.isSecureContext && window.PasswordCredential && navigator.credentials?.store) {
                const username = [...this.fields.values()].find(entry => entry.input.autocomplete === "username");
                const password = [...this.fields.values()].find(entry => entry.input.autocomplete === "new-password")
                    || [...this.fields.values()].find(entry => entry.input.autocomplete === "current-password");
                if (username?.input.value && password?.input.value) {
                    const credential = new PasswordCredential({ id: username.input.value, password: password.input.value });
                    navigator.credentials.store(credential).catch(() => {});
                }
            }
        }
        else for (const entry of this.fields.values()) {
            entry.input.value = entry.value;
            entry.pending = null;
        }
    }

    dispose() {
        this.disposed = true;
        for (const entry of this.fields.values()) entry.input.value = "";
        this.fields.clear();
        this.form.remove();
    }
}
