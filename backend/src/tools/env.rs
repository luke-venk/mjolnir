use std::env;
use std::str::FromStr;

pub fn read_env_string(key: &str) -> Option<String> {
    env::var(key).ok().filter(|v| !v.trim().is_empty())
}


pub fn read_env_bool(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(v) => match v.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        },
        Err(_) => default,
    }
}

pub fn read_env_number<T>(key: &str) -> Option<T>
where
    T: FromStr,
{
    env::var(key).ok()?.trim().parse::<T>().ok()
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn read_env_string_returns_none_for_missing() {
        let key = "TEST_READ_ENV_STRING_MISSING";
        unsafe {
            env::remove_var(key);
        }
        assert_eq!(read_env_string(key), None);
    }

    #[test]
    fn read_env_string_returns_none_for_blank() {
        let key = "TEST_READ_ENV_STRING_BLANK";
        unsafe {
            env::set_var(key, "   ");
        }
        assert_eq!(read_env_string(key), None);
        unsafe {
            env::remove_var(key);
        }
    }

    #[test]
    fn read_env_string_returns_value() {
        let key = "TEST_READ_ENV_STRING_VALUE";
        unsafe {
            env::set_var(key, "camera-123");
        }
        assert_eq!(read_env_string(key), Some("camera-123".to_string()));
        unsafe {
            env::remove_var(key);
        }
    }

    #[test]
    fn read_env_number_parses_f64() {
        let key = "TEST_READ_ENV_NUMBER_F64";
        unsafe {
            env::set_var(key, "12.5");
        }
        assert_eq!(read_env_number::<f64>(key), Some(12.5));
        unsafe {
            env::remove_var(key);
        }
    }

    #[test]
    fn read_env_number_parses_usize() {
        let key = "TEST_READ_ENV_NUMBER_USIZE";
        unsafe {
            env::set_var(key, "16");
        }
        assert_eq!(read_env_number::<usize>(key), Some(16));
        unsafe {
            env::remove_var(key);
        }
    }

    #[test]
    fn read_env_number_returns_none_for_invalid() {
        let key = "TEST_READ_ENV_NUMBER_INVALID";
        unsafe {
            env::set_var(key, "not-a-number");
        }
        assert_eq!(read_env_number::<u64>(key), None);
        unsafe {
            env::remove_var(key);
        }
    }

    #[test]
    fn read_env_bool_returns_default_for_missing() {
        let key = "TEST_READ_ENV_BOOL_MISSING";
        unsafe {
            env::remove_var(key);
        }
        assert!(read_env_bool(key, true));
        assert!(!read_env_bool(key, false));
    }

    #[test]
    fn read_env_bool_parses_true_values() {
        let key = "TEST_READ_ENV_BOOL_TRUE";
        unsafe {
            env::set_var(key, "true");
        }
        assert!(read_env_bool(key, false));
        unsafe {
            env::remove_var(key);
        }
    }

    #[test]
    fn read_env_bool_parses_false_values() {
        let key = "TEST_READ_ENV_BOOL_FALSE";
        unsafe {
            env::set_var(key, "false");
        }
        assert!(!read_env_bool(key, true));
        unsafe {
            env::remove_var(key);
        }
    }

    #[test]
    fn read_env_bool_returns_default_for_invalid() {
        let key = "TEST_READ_ENV_BOOL_INVALID";
        unsafe {
            env::set_var(key, "banana");
        }
        assert!(read_env_bool(key, true));
        assert!(!read_env_bool(key, false));
        unsafe {
            env::remove_var(key);
        }
    }
}