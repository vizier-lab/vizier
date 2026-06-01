use crate::schema::Skill;

pub fn match_skills_by_keywords<'a>(skills: &'a [Skill], task: &str) -> Vec<&'a Skill> {
    let task_lower = task.to_lowercase();

    skills
        .iter()
        .filter(|skill| {
            skill
                .keywords
                .iter()
                .any(|keyword| task_lower.contains(&keyword.to_lowercase()))
        })
        .collect()
}

pub fn match_skills_by_description<'a>(skills: &'a [Skill], task: &str) -> Vec<&'a Skill> {
    let task_lower = task.to_lowercase();
    let task_words: Vec<&str> = task_lower.split_whitespace().collect();

    let mut scored: Vec<(&Skill, f64)> = skills
        .iter()
        .filter(|skill| {
            !skill.keywords.is_empty() || !skill.description.is_empty()
        })
        .map(|skill| {
            let desc_lower = skill.description.to_lowercase();
            let name_lower = skill.name.to_lowercase();

            let mut score = 0.0;

            // Check description
            for word in &task_words {
                if desc_lower.contains(word) {
                    score += 1.0;
                }
            }

            // Check name
            for word in &task_words {
                if name_lower.contains(word) {
                    score += 2.0;
                }
            }

            // Check keywords with fuzzy matching
            for keyword in &skill.keywords {
                let keyword_lower = keyword.to_lowercase();
                for word in &task_words {
                    if word.contains(&keyword_lower) || keyword_lower.contains(word) {
                        score += 3.0;
                    }
                }
            }

            (skill, score)
        })
        .filter(|(_, score)| *score > 0.0)
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().map(|(skill, _)| skill).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::SkillActivation;

    fn make_skill(name: &str, keywords: Vec<&str>, description: &str) -> Skill {
        Skill {
            name: name.to_string(),
            agent_id: None,
            author: "test".to_string(),
            description: description.to_string(),
            content: String::new(),
            keywords: keywords.into_iter().map(String::from).collect(),
            activation: SkillActivation::Contextual,
            version: 1,
            resources: Vec::new(),
        }
    }

    #[test]
    fn test_keyword_matching() {
        let skills = vec![
            make_skill("code-review", vec!["review", "quality", "security"], "Guidelines for code reviews"),
            make_skill("calculator", vec!["math", "calculate", "compute"], "Perform mathematical calculations"),
            make_skill("deploy", vec!["deploy", "release", "ship"], "Deployment instructions"),
        ];

        let matched = match_skills_by_keywords(&skills, "please review this PR for security issues");
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].name, "code-review");
    }

    #[test]
    fn test_multiple_keyword_matches() {
        let skills = vec![
            make_skill("code-review", vec!["review", "quality"], "Code review guidelines"),
            make_skill("security-audit", vec!["security", "audit"], "Security audit checklist"),
        ];

        let matched = match_skills_by_keywords(&skills, "review this for security");
        assert_eq!(matched.len(), 2);
    }

    #[test]
    fn test_no_matches() {
        let skills = vec![
            make_skill("code-review", vec!["review", "quality"], "Code review guidelines"),
        ];

        let matched = match_skills_by_keywords(&skills, "what is the weather today?");
        assert!(matched.is_empty());
    }

    #[test]
    fn test_description_matching() {
        let skills = vec![
            make_skill("code-review", vec![], "Guidelines for conducting code reviews"),
            make_skill("deploy", vec![], "Instructions for deploying applications"),
        ];

        let matched = match_skills_by_description(&skills, "how do I review code?");
        assert!(!matched.is_empty());
        assert_eq!(matched[0].name, "code-review");
    }
}
