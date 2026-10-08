use crate::bridge::{call, window};
use gpui::{AutofillField, AutofillHint};
use jni::{
    JNIEnv,
    objects::{JClass, JString},
    sys::jlong,
};

pub(crate) fn encode(fields: &[AutofillField], scale: f32) -> String {
    let fields: Vec<_> = fields.iter().map(|field| {
        let hint = match field.hint {
            AutofillHint::Username => "username",
            AutofillHint::Password => "password",
            AutofillHint::NewUsername => "newUsername",
            AutofillHint::NewPassword => "newPassword",
            AutofillHint::Email => "emailAddress",
            AutofillHint::Name => "personName",
            AutofillHint::GivenName => "personGivenName",
            AutofillHint::FamilyName => "personFamilyName",
            AutofillHint::Phone => "phoneNumber",
            AutofillHint::PostalAddress => "postalAddress",
            AutofillHint::PostalCode => "postalCode",
            AutofillHint::OneTimeCode => "smsOTPCode",
        };
        serde_json::json!({
            "id": field.id.to_string(), "name": field.name.as_ref(), "hint": hint, "value": field.value.as_ref(),
            "focused": field.focused,
            "x": f32::from(field.bounds.left()) * scale,
            "y": f32::from(field.bounds.top()) * scale,
            "width": f32::from(field.bounds.size.width) * scale,
            "height": f32::from(field.bounds.size.height) * scale,
        })
    }).collect();
    serde_json::to_string(&fields).expect("autofill fields serialize")
}

pub(crate) extern "system" fn fill(
    mut env: JNIEnv,
    _: JClass,
    session: jlong,
    id: JString,
    value: JString,
) {
    call(&mut env, |env| {
        let id: u64 = String::from(env.get_string(&id)?).parse()?;
        let value = String::from(env.get_string(&value)?);
        let window = window(session)?;
        if window
            .autofill_fields
            .borrow()
            .iter()
            .any(|field| field.id == id)
        {
            if let Some(callback) = window.autofill_callback.borrow().as_ref() {
                callback(id, value);
            }
        }
        Ok(())
    });
}
