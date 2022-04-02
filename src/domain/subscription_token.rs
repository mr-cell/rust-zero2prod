use validator::validate_length;

#[derive(Debug)]
pub struct SubscriptionToken(String);

impl AsRef<str> for SubscriptionToken {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl SubscriptionToken {
    pub fn parse(s: String) -> Result<SubscriptionToken, String> {
        if validate_token(s.as_str()) {
            Ok(Self(s))
        } else {
            Err(format!("{} is not a valid subscription token", s))
        }
    }
}

fn validate_token(s: &str) -> bool {
    validate_length(s, None, None, Some(25)) && s.chars().all(|c| c.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use crate::domain::subscription_token::SubscriptionToken;
    use claim::{assert_err, assert_ok};
    use fake::{Fake, StringFaker};

    const ALPHANUMERIC: &str = "abcdefghijklmnoperqstuvwxyzABCDEFGHIJKLMNOPRQSTUVWXYZ1234567890";
    const SPECIAL_CHARS: &str = "£§!@#$%^&*()_+{}[]-=~`<,>.?/:;\"' ";

    #[test]
    fn token_with_special_chars_is_invalid() {
        let token: String = StringFaker::with(Vec::from(SPECIAL_CHARS), 25).fake();
        assert_err!(SubscriptionToken::parse(token));
    }

    #[test]
    fn alphanumeric_token_of_length_25_is_valid() {
        let token: String = StringFaker::with(Vec::from(ALPHANUMERIC), 25).fake();
        assert_ok!(SubscriptionToken::parse(token));
    }

    #[test]
    fn alphanumeric_token_of_length_other_than_25_is_invalid() {
        let token: String = StringFaker::with(Vec::from(ALPHANUMERIC), 0..24).fake();
        assert_err!(SubscriptionToken::parse(token));

        let token: String = StringFaker::with(Vec::from(ALPHANUMERIC), 26..50).fake();
        assert_err!(SubscriptionToken::parse(token));
    }
}
