use super::{
    contains_receiver_path, contains_simple_assignment_to, is_receiver_path_char, let_binding_name,
};

pub(super) fn has_initialized_range_evidence(
    before_call: &str,
    same_vec_target: &str,
    set_len_argument: &str,
) -> bool {
    SetLenInitializedRangeContext {
        before_call,
        same_vec_target,
        set_len_argument,
    }
    .has_initialized_range_evidence()
}

struct SetLenInitializedRangeContext<'a> {
    before_call: &'a str,
    same_vec_target: &'a str,
    set_len_argument: &'a str,
}

impl<'a> SetLenInitializedRangeContext<'a> {
    fn has_initialized_range_evidence(&self) -> bool {
        self.has_initialization_loop()
    }

    fn has_initialization_loop(&self) -> bool {
        let slice_bindings = self.slice_bindings();
        self.before_call.split('}').any(|block| {
            let Some((head, body)) = block.rsplit_once('{') else {
                return false;
            };
            self.loop_initializes_same_vec(head, body, &slice_bindings)
        })
    }

    fn loop_initializes_same_vec(&self, head: &str, body: &str, slice_bindings: &[&str]) -> bool {
        self.loop_iterates_receiver(head, slice_bindings)
            && head.contains(".iter_mut(")
            && has_initialization_marker(body)
    }

    fn slice_bindings(&self) -> Vec<&'a str> {
        let mut bindings = Vec::new();
        let mut consumed = 0usize;
        for statement in self.before_call.split_inclusive(';') {
            let statement_without_semicolon = statement.trim_end_matches(';');
            let after_binding =
                &self.before_call[(consumed + statement.len()).min(self.before_call.len())..];
            if let Some(binding) =
                self.fresh_slice_binding(statement_without_semicolon, after_binding)
            {
                bindings.push(binding);
            }
            consumed += statement.len();
        }
        bindings
    }

    fn fresh_slice_binding(&self, statement: &'a str, after_binding: &str) -> Option<&'a str> {
        let (left, right) = statement.split_once('=')?;
        let binding = let_binding_name(left)?;
        let right = right.trim();
        (set_len_slice_binding_references_receiver(right, self.same_vec_target)
            && set_len_slice_binding_covers_argument(right, self.set_len_argument)
            && right.contains('[')
            && right.contains("..")
            && !contains_simple_assignment_to(after_binding, self.same_vec_target)
            && !contains_direct_binding_assignment_to(after_binding, binding))
        .then_some(binding)
    }

    fn loop_iterates_receiver(&self, head: &str, slice_bindings: &[&str]) -> bool {
        contains_receiver_path(head, self.same_vec_target)
            || head.contains(&format!("in{}.", self.same_vec_target))
            || slice_bindings.iter().any(|binding| {
                contains_receiver_path(head, binding) || head.contains(&format!("in{binding}."))
            })
    }
}

fn set_len_slice_binding_references_receiver(right: &str, receiver: &str) -> bool {
    let right = right
        .strip_prefix("&mut")
        .or_else(|| right.strip_prefix('&'))
        .unwrap_or(right);
    contains_receiver_path(right, receiver)
}

fn set_len_slice_binding_covers_argument(right: &str, set_len_argument: &str) -> bool {
    let right = right
        .strip_prefix("&mut")
        .or_else(|| right.strip_prefix('&'))
        .unwrap_or(right);
    let Some(range_start) = right.find('[') else {
        return false;
    };
    let range = &right[range_start + 1..];
    let Some(range_end) = range.find(']') else {
        return false;
    };
    let range = &range[..range_end];
    let Some((_start, end)) = range.split_once("..") else {
        return false;
    };
    end == set_len_argument
}

fn contains_direct_binding_assignment_to(compact: &str, name: &str) -> bool {
    if compact.contains(&format!("let{name}="))
        || compact.contains(&format!("letmut{name}="))
        || compact.contains(&format!("let{name}:"))
        || compact.contains(&format!("letmut{name}:"))
    {
        return true;
    }
    let marker = format!("{name}=");
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(&marker) {
        let start = offset + pos;
        let before = compact[..start].chars().next_back();
        let after_equals = compact[start + marker.len()..].chars().next();
        if before.is_none_or(|ch| ch != '*' && !is_receiver_path_char(ch))
            && after_equals != Some('=')
        {
            return true;
        }
        let next = pos + marker.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn has_initialization_marker(statement: &str) -> bool {
    statement.contains("maybeuninit::new")
        || statement.contains(".write(")
        || statement.contains("ptr::write")
        || statement.contains("copy_nonoverlapping")
        || statement.contains("copy_to_nonoverlapping")
}
