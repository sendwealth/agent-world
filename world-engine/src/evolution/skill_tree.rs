//! Skill tree with branching paths and XP-based leveling.
//!
//! Each skill belongs to a **branch** (e.g. `coding` has sub-branches
//! `frontend`, `backend`, `systems`). Skills have levels 1–10 with
//! exponentially growing XP thresholds.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A branch in the skill tree (e.g. "coding" → ["frontend", "backend", "systems"]).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillBranch {
    /// Root skill name (e.g. "coding").
    pub root: String,
    /// Sub-branch names (e.g. ["frontend", "backend", "systems"]).
    pub sub_branches: Vec<String>,
}

/// A single node in the skill tree, representing one skill at a given level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillNode {
    /// Skill name (e.g. "frontend").
    pub name: String,
    /// Parent skill, if any (e.g. "coding").
    pub parent: Option<String>,
    /// Current level (1–10).
    pub level: u32,
    /// Accumulated experience points.
    pub experience: f64,
    /// Maximum level allowed.
    pub max_level: u32,
}

impl SkillNode {
    /// Create a new skill node at level 1 with 0 XP.
    pub fn new(name: String, parent: Option<String>, max_level: u32) -> Self {
        Self {
            name,
            parent,
            level: 1,
            experience: 0.0,
            max_level,
        }
    }

    /// XP threshold for the *next* level, using exponential growth.
    ///
    /// Formula: `100 * 2^(level - 1)`. Level 1→2 requires 100 XP,
    /// level 2→3 requires 200 XP, level 9→10 requires 25600 XP.
    pub fn xp_threshold_for_level(level: u32) -> f64 {
        100.0 * 2_f64.powi(level as i32 - 1)
    }

    /// XP threshold needed to advance from the current level to the next.
    pub fn xp_to_next_level(&self) -> f64 {
        if self.level >= self.max_level {
            f64::INFINITY
        } else {
            Self::xp_threshold_for_level(self.level)
        }
    }

    /// Add XP and level up if threshold is crossed. Returns `true` if leveled up.
    pub fn add_experience(&mut self, xp: f64) -> bool {
        if self.level >= self.max_level {
            return false;
        }

        self.experience += xp;

        let mut leveled_up = false;
        while self.level < self.max_level
            && self.experience >= Self::xp_threshold_for_level(self.level)
        {
            self.experience -= Self::xp_threshold_for_level(self.level);
            self.level += 1;
            leveled_up = true;
        }

        leveled_up
    }
}

/// The skill tree registry, defining all available branches and skills.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillTree {
    /// All registered branches.
    pub branches: Vec<SkillBranch>,
    /// Maximum skill level (default 10).
    pub max_level: u32,
}

impl SkillTree {
    /// Create a skill tree with the default branch definitions.
    pub fn new(max_level: u32) -> Self {
        Self {
            branches: vec![
                SkillBranch {
                    root: "coding".into(),
                    sub_branches: vec!["frontend".into(), "backend".into(), "systems".into()],
                },
                SkillBranch {
                    root: "communication".into(),
                    sub_branches: vec!["negotiation".into(), "teaching".into()],
                },
                SkillBranch {
                    root: "survival".into(),
                    sub_branches: vec!["resource_gathering".into(), "risk_assessment".into()],
                },
                SkillBranch {
                    root: "social".into(),
                    sub_branches: vec!["networking".into(), "leadership".into()],
                },
            ],
            max_level,
        }
    }

    /// List all skill names (roots + sub-branches).
    pub fn all_skill_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for branch in &self.branches {
            names.push(branch.root.clone());
            for sub in &branch.sub_branches {
                names.push(sub.clone());
            }
        }
        names
    }

    /// List only sub-branch skill names (excludes roots).
    pub fn sub_branch_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for branch in &self.branches {
            for sub in &branch.sub_branches {
                names.push(sub.clone());
            }
        }
        names
    }

    /// Get the parent for a given skill name, if any.
    pub fn parent_of(&self, skill_name: &str) -> Option<String> {
        for branch in &self.branches {
            if branch.root == skill_name {
                return None; // root has no parent
            }
            if branch.sub_branches.iter().any(|s| s == skill_name) {
                return Some(branch.root.clone());
            }
        }
        None
    }

    /// Get the sub-branch names for a given root skill.
    pub fn sub_branches_of(&self, root: &str) -> Vec<String> {
        self.branches
            .iter()
            .find(|b| b.root == root)
            .map(|b| b.sub_branches.clone())
            .unwrap_or_default()
    }

    /// Build a map of `SkillNode`s from an agent's existing skill experience.
    ///
    /// Any skill not present in `existing` is created at level 1 with 0 XP.
    pub fn build_nodes(
        &self,
        existing: &HashMap<String, (u32, f64)>,
    ) -> HashMap<String, SkillNode> {
        let mut nodes = HashMap::new();
        for name in self.all_skill_names() {
            let parent = self.parent_of(&name);
            if let Some((level, xp)) = existing.get(&name) {
                nodes.insert(
                    name.clone(),
                    SkillNode {
                        name: name.clone(),
                        parent,
                        level: *level,
                        experience: *xp,
                        max_level: self.max_level,
                    },
                );
            } else {
                nodes.insert(name.clone(), SkillNode::new(name, parent, self.max_level));
            }
        }
        nodes
    }
}

impl Default for SkillTree {
    fn default() -> Self {
        Self::new(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xp_threshold_grows_exponentially() {
        assert_eq!(SkillNode::xp_threshold_for_level(1), 100.0);
        assert_eq!(SkillNode::xp_threshold_for_level(2), 200.0);
        assert_eq!(SkillNode::xp_threshold_for_level(3), 400.0);
        assert_eq!(SkillNode::xp_threshold_for_level(9), 25600.0);
    }

    #[test]
    fn add_xp_levels_up_once() {
        let mut node = SkillNode::new("frontend".into(), Some("coding".into()), 10);
        assert_eq!(node.level, 1);
        assert!(node.add_experience(150.0));
        assert_eq!(node.level, 2);
        assert!((node.experience - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn add_xp_levels_up_multiple() {
        let mut node = SkillNode::new("frontend".into(), Some("coding".into()), 10);
        // 100 + 200 = 300 XP needed for level 1→3
        assert!(node.add_experience(350.0));
        assert_eq!(node.level, 3);
        assert!((node.experience - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn no_level_up_at_max() {
        let mut node = SkillNode::new("frontend".into(), Some("coding".into()), 2);
        assert!(node.add_experience(100.0)); // 1→2
        assert_eq!(node.level, 2);
        assert!(!node.add_experience(9999.0)); // maxed out
        assert_eq!(node.level, 2);
    }

    #[test]
    fn no_level_up_insufficient_xp() {
        let mut node = SkillNode::new("frontend".into(), Some("coding".into()), 10);
        assert!(!node.add_experience(50.0));
        assert_eq!(node.level, 1);
        assert!((node.experience - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn skill_tree_default_branches() {
        let tree = SkillTree::default();
        assert_eq!(tree.branches.len(), 4);
        assert_eq!(tree.branches[0].root, "coding");
        assert_eq!(
            tree.branches[0].sub_branches,
            vec!["frontend", "backend", "systems"]
        );
    }

    #[test]
    fn skill_tree_parent_of() {
        let tree = SkillTree::default();
        assert!(tree.parent_of("coding").is_none());
        assert_eq!(tree.parent_of("frontend"), Some("coding".into()));
        assert_eq!(tree.parent_of("negotiation"), Some("communication".into()));
    }

    #[test]
    fn skill_tree_sub_branches_of() {
        let tree = SkillTree::default();
        assert_eq!(
            tree.sub_branches_of("coding"),
            vec!["frontend", "backend", "systems"],
        );
        assert!(tree.sub_branches_of("nonexistent").is_empty());
    }

    #[test]
    fn build_nodes_from_existing() {
        let tree = SkillTree::new(10);
        let mut existing = HashMap::new();
        existing.insert("coding".into(), (5, 50.0));
        let nodes = tree.build_nodes(&existing);
        assert_eq!(nodes["coding"].level, 5);
        assert_eq!(nodes["coding"].experience, 50.0);
        // Sub-branches default to level 1
        assert_eq!(nodes["frontend"].level, 1);
    }

    #[test]
    fn all_skill_names_includes_roots_and_subs() {
        let tree = SkillTree::default();
        let names = tree.all_skill_names();
        assert!(names.contains(&"coding".to_string()));
        assert!(names.contains(&"frontend".to_string()));
        assert!(names.contains(&"networking".to_string()));
    }

    #[test]
    fn sub_branch_names_excludes_roots() {
        let tree = SkillTree::default();
        let names = tree.sub_branch_names();
        assert!(!names.contains(&"coding".to_string()));
        assert!(names.contains(&"frontend".to_string()));
    }
}
