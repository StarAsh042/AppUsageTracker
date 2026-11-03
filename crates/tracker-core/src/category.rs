use crate::settings::CategoryRuleDef;

pub const UNCATEGORIZED: &str = "未分类";

pub struct CategoryResolver {
    rules: Vec<CategoryRuleDef>,
    overrides: std::collections::HashMap<String, String>,
}

impl CategoryResolver {
    pub fn new(
        rules: Vec<CategoryRuleDef>,
        overrides: std::collections::HashMap<String, String>,
    ) -> Self {
        Self { rules, overrides }
    }

    pub fn resolve(&self, display_name: &str, process_name: &str, exe_path: &str) -> String {
        if let Some(cat) = self.overrides.get(&exe_path.to_lowercase()) {
            return cat.clone();
        }
        let haystack = format!("{}\n{}\n{}", display_name, process_name, exe_path).to_lowercase();
        for rule in &self.rules {
            for kw in &rule.keywords {
                if !kw.is_empty() && haystack.to_lowercase().contains(&kw.to_lowercase()) {
                    return rule.name.clone();
                }
            }
        }
        UNCATEGORIZED.to_string()
    }
}
