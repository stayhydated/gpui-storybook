use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SidebarStoryMetadata {
    pub(super) title: String,
    pub(super) group: Option<String>,
    pub(super) section: Option<String>,
}

pub(super) type SidebarStoryGroups = BTreeMap<Option<String>, BTreeMap<Option<String>, Vec<usize>>>;

pub(super) fn group_matching_stories(
    stories: &[SidebarStoryMetadata],
    query: &str,
) -> SidebarStoryGroups {
    let query = query.trim().to_lowercase();
    let mut groups = SidebarStoryGroups::new();

    for (index, story) in stories.iter().enumerate() {
        let matches = query.is_empty()
            || story.title.to_lowercase().contains(&query)
            || story
                .group
                .as_deref()
                .is_some_and(|group| group.to_lowercase().contains(&query))
            || story
                .section
                .as_deref()
                .is_some_and(|section| section.to_lowercase().contains(&query));
        if !matches {
            continue;
        }

        groups
            .entry(story.group.clone())
            .or_default()
            .entry(story.section.clone())
            .or_default()
            .push(index);
    }

    groups
}

#[cfg(test)]
mod tests {
    use super::{SidebarStoryMetadata, group_matching_stories};

    fn story(title: &str, group: Option<&str>, section: Option<&str>) -> SidebarStoryMetadata {
        SidebarStoryMetadata {
            title: title.to_string(),
            group: group.map(str::to_string),
            section: section.map(str::to_string),
        }
    }

    #[test]
    fn groups_stories_by_group_and_section_without_losing_order() {
        let stories = [
            story("Primary Button", Some("Inputs"), Some("Buttons")),
            story("Text Input", Some("Inputs"), Some("Fields")),
            story("Secondary Button", Some("Inputs"), Some("Buttons")),
            story("Welcome", None, None),
        ];

        let groups = group_matching_stories(&stories, "");

        assert_eq!(
            groups[&Some("Inputs".to_string())][&Some("Buttons".to_string())],
            [0, 2]
        );
        assert_eq!(
            groups[&Some("Inputs".to_string())][&Some("Fields".to_string())],
            [1]
        );
        assert_eq!(groups[&None][&None], [3]);
    }

    #[test]
    fn matches_titles_groups_and_sections_case_insensitively() {
        let stories = [
            story("Primary Button", Some("Inputs"), Some("Buttons")),
            story("Data Grid", Some("Tables"), Some("Results")),
            story("Welcome", None, None),
        ];

        assert_eq!(
            group_matching_stories(&stories, " primary ")[&Some("Inputs".to_string())]
                [&Some("Buttons".to_string())],
            [0]
        );
        assert_eq!(
            group_matching_stories(&stories, "TABLES")[&Some("Tables".to_string())]
                [&Some("Results".to_string())],
            [1]
        );
        assert_eq!(
            group_matching_stories(&stories, "results")[&Some("Tables".to_string())]
                [&Some("Results".to_string())],
            [1]
        );
        assert!(group_matching_stories(&stories, "missing").is_empty());
    }
}
