use crate::{Bounds, Pixels, SharedString};

/// The semantic content of a text field offered to a system autofill service.
/// This does not change the keyboard layout or validate entered text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutofillHint {
    /// An existing account identifier.
    Username,
    /// An existing account's password.
    Password,
    /// An identifier being registered for a new account.
    NewUsername,
    /// A password being created or changed.
    NewPassword,
    /// An email address.
    Email,
    /// A person's full name.
    Name,
    /// A person's given name.
    GivenName,
    /// A person's family name.
    FamilyName,
    /// A telephone number.
    Phone,
    /// A complete postal address.
    PostalAddress,
    /// A postal or ZIP code.
    PostalCode,
    /// A one-time verification code.
    OneTimeCode,
}

impl AutofillHint {
    /// The HTML autocomplete token for this field's semantic purpose.
    pub fn autocomplete(self) -> &'static str {
        match self {
            Self::Username | Self::NewUsername => "username",
            Self::Password => "current-password",
            Self::NewPassword => "new-password",
            Self::Email => "email",
            Self::Name => "name",
            Self::GivenName => "given-name",
            Self::FamilyName => "family-name",
            Self::Phone => "tel",
            Self::PostalAddress => "street-address",
            Self::PostalCode => "postal-code",
            Self::OneTimeCode => "one-time-code",
        }
    }
}

/// Stable identity and content semantics for an autofill field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutofillOptions {
    /// A stable, nonempty name, unique among the window's autofill fields.
    pub name: SharedString,
    /// The type of information expected by this field.
    pub hint: AutofillHint,
}

impl AutofillOptions {
    /// Describes a field without relying on placeholder text or label heuristics.
    pub fn new(name: impl Into<SharedString>, hint: AutofillHint) -> Self {
        Self {
            name: name.into(),
            hint,
        }
    }
}

/// A rendered text field exposed to the platform autofill adapter.
/// Values may contain secrets and must not be logged or exposed through accessibility.
#[derive(Clone, PartialEq)]
pub struct AutofillField {
    /// Stable identity for this field's lifetime.
    pub id: u64,
    /// Stable application-provided name, independent of the transient numeric ID.
    pub name: SharedString,
    /// The field's semantic purpose.
    pub hint: AutofillHint,
    /// The complete current value, including for password fields.
    pub value: SharedString,
    /// Displayed bounds in logical window coordinates.
    pub bounds: Bounds<Pixels>,
    /// Whether this field currently has text focus.
    pub focused: bool,
}
