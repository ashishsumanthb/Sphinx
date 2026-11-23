// Demo encoding only. Anyone knowing this formula can reverse it.

pub fn encode(password: &str) -> String {
    password
        .bytes()
        .map(|byte| {
            let number = u32::from(byte) + 1000;
            let password_quotient = number / 26;
            let password_remainder = number % 26;

            format!("{password_quotient}:{password_remainder}")
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub fn decode(encoded: &str) -> Result<String, &'static str> {
    if encoded.is_empty() {
        return Ok(String::new());
    }

    let mut bytes = Vec::new();

    for part in encoded.split(',') {
        let (password_quotient, password_remainder) =
            part.split_once(':').ok_or("Invalid encoded format")?;

        let password_quotient: u32 = password_quotient.parse().map_err(|_| "Invalid quotient")?;

        let password_remainder: u32 = password_remainder
            .parse()
            .map_err(|_| "Invalid remainder")?;

        if password_remainder >= 26 {
            return Err("Invalid remainder");
        }

        let byte = password_quotient
            .checked_mul(26)
            .and_then(|number| number.checked_add(password_remainder))
            .and_then(|number| number.checked_sub(1000))
            .and_then(|number| u8::try_from(number).ok())
            .ok_or("Invalid encoded byte")?;

        bytes.push(byte);
    }

    String::from_utf8(bytes).map_err(|_| "Invalid UTF-8 text")
}

#[cfg(test)]
mod tests {
    use super::{decode, encode};

    #[test]
    fn encodes_known_text() {
        assert_eq!(encode("AB"), "40:25,41:0");
        assert_eq!(decode("40:25,41:0"), Ok(String::from("AB")));
    }

    #[test]
    fn restores_different_text_inputs() {
        for password in [
            "",
            "00123",
            "Hello@123!",
            " spaces stay ",
            "తెలుగు",
            "🔐🙂",
            "a,b:c",
        ] {
            assert_eq!(decode(&encode(password)).unwrap(), password);
        }
    }

    #[test]
    fn rejects_invalid_encoded_values() {
        for encoded in ["abc", "38:26", "0:0", "4294967295:0", "48:7"] {
            assert!(decode(encoded).is_err());
        }
    }
}
