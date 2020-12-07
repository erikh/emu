use std::collections::HashMap;

pub type Ini = HashMap<String, HashMap<String, Option<String>>>;

pub fn to_ini(ini: &Ini) -> String {
    let mut s = String::new();

    for (section, v) in ini {
        s += &format!("[{}]\n", section);

        for (key, value) in v {
            match value {
                Some(value) => s += &format!("{} = {}\n", key, value),
                None => s += &format!("{}\n", key),
            }
        }
    }

    s
}
