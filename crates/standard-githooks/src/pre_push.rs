/// Return whether Git's `pre-push` input contains only ref deletions.
///
/// Empty or malformed input is not classified as a deletion-only push.
pub fn is_deletion_only(input: &str) -> bool {
    let mut object_name_width = None;
    let mut has_update = false;

    for line in input.lines() {
        has_update = true;
        let Some(width) = deletion_record_width(line) else {
            return false;
        };
        match object_name_width {
            Some(expected) if width != expected => return false,
            None => object_name_width = Some(width),
            Some(_) => {}
        }
    }

    has_update
}

fn deletion_record_width(line: &str) -> Option<usize> {
    let mut fields = line.split_whitespace();
    let local_ref = fields.next()?;
    let local_object = fields.next()?;
    let _remote_ref = fields.next()?;
    let remote_object = fields.next()?;

    (local_ref == "(delete)"
        && is_zero_object_name(local_object)
        && is_object_name(remote_object)
        && local_object.len() == remote_object.len()
        && fields.next().is_none())
    .then_some(local_object.len())
}

fn is_zero_object_name(name: &str) -> bool {
    matches!(name.len(), 40 | 64) && name.bytes().all(|byte| byte == b'0')
}

fn is_object_name(name: &str) -> bool {
    matches!(name.len(), 40 | 64) && name.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_a_deletion_only_push() {
        let input = "(delete) 0000000000000000000000000000000000000000 \
                     refs/heads/topic 1111111111111111111111111111111111111111\n\
                     (delete) 0000000000000000000000000000000000000000 \
                     refs/heads/other 2222222222222222222222222222222222222222\n";

        assert!(is_deletion_only(input));
    }

    #[test]
    fn rejects_a_malformed_deletion_record() {
        let input = "(delete) 0 refs/heads/topic 1111111111111111111111111111111111111111\n";

        assert!(!is_deletion_only(input));

        let input = "(delete) 0000000000000000000000000000000000000000 refs/heads/topic invalid\n";

        assert!(!is_deletion_only(input));

        let input = "refs/heads/topic 0000000000000000000000000000000000000000 \
                     refs/heads/topic 1111111111111111111111111111111111111111\n";
        assert!(!is_deletion_only(input));

        let input = "(delete) 1000000000000000000000000000000000000000 \
                     refs/heads/topic 1111111111111111111111111111111111111111\n";
        assert!(!is_deletion_only(input));

        let input = "(delete) 0000000000000000000000000000000000000000 \
                     refs/heads/topic 1111111111111111111111111111111111111111 extra\n";
        assert!(!is_deletion_only(input));
    }

    #[test]
    fn blank_record_is_malformed() {
        let input = "(delete) 0000000000000000000000000000000000000000 \
                     refs/heads/topic 1111111111111111111111111111111111111111\n\n";

        assert!(!is_deletion_only(input));
    }

    #[test]
    fn object_names_must_use_the_same_width() {
        let input = "(delete) 0000000000000000000000000000000000000000 \
                     refs/heads/topic \
                     1111111111111111111111111111111111111111111111111111111111111111\n";

        assert!(!is_deletion_only(input));

        let input = "(delete) 0000000000000000000000000000000000000000 \
                     refs/heads/sha1 1111111111111111111111111111111111111111\n\
                     (delete) \
                     0000000000000000000000000000000000000000000000000000000000000000 \
                     refs/heads/sha256 \
                     1111111111111111111111111111111111111111111111111111111111111111\n";
        assert!(!is_deletion_only(input));
    }

    #[test]
    fn classifies_sha256_deletions() {
        let input = "(delete) \
                     0000000000000000000000000000000000000000000000000000000000000000 \
                     refs/heads/topic \
                     1111111111111111111111111111111111111111111111111111111111111111\n";

        assert!(is_deletion_only(input));
    }

    #[test]
    fn does_not_classify_a_mixed_push_as_deletion_only() {
        let input = "(delete) 0000000000000000000000000000000000000000 \
                     refs/heads/old 1111111111111111111111111111111111111111\n\
                     refs/heads/main 2222222222222222222222222222222222222222 \
                     refs/heads/main 1111111111111111111111111111111111111111\n";

        assert!(!is_deletion_only(input));
    }

    #[test]
    fn empty_input_is_not_deletion_only() {
        assert!(!is_deletion_only(""));
    }
}
