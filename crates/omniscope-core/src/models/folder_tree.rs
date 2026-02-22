use std::collections::HashMap;

use crate::models::{Folder, FolderType};

#[derive(Debug, Clone)]
pub struct FolderNode {
    pub folder: Folder,
    pub children: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FolderTree {
    pub nodes: HashMap<String, FolderNode>,
    pub root_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum FolderTreeChange {
    Added(Folder),
    Updated(Folder),
    Deleted(String), // By ID
}

impl FolderTree {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            root_ids: Vec::new(),
        }
    }

    pub fn build(folders: Vec<Folder>) -> Self {
        let mut tree = Self::new();
        
        // 1. Insert all nodes
        for folder in folders {
            tree.nodes.insert(
                folder.id.clone(),
                FolderNode {
                    folder,
                    children: Vec::new(),
                },
            );
        }

        // 2. Link children and identify roots
        let mut root_ids = Vec::new();
        let mut parent_to_children: HashMap<String, Vec<String>> = HashMap::new();

        for (id, node) in &tree.nodes {
            if let Some(parent_id) = &node.folder.parent_id {
                parent_to_children
                    .entry(parent_id.clone())
                    .or_default()
                    .push(id.clone());
            } else {
                root_ids.push(id.clone());
            }
        }

        for (parent_id, mut children) in parent_to_children {
            Self::sort_children(&mut children, &tree.nodes);
            if let Some(parent_node) = tree.nodes.get_mut(&parent_id) {
                parent_node.children = children;
            } else {
                root_ids.extend(children);
            }
        }

        Self::sort_children(&mut root_ids, &tree.nodes);
        tree.root_ids = root_ids;

        tree
    }

    pub fn apply_change(&self, change: FolderTreeChange) -> Self {
        let mut new_tree = self.clone();

        match change {
            FolderTreeChange::Added(folder) => {
                let id = folder.id.clone();
                let parent_id = folder.parent_id.clone();
                
                new_tree.nodes.insert(
                    id.clone(),
                    FolderNode {
                        folder,
                        children: Vec::new(),
                    },
                );

                if let Some(parent_id) = parent_id {
                    if new_tree.nodes.contains_key(&parent_id) {
                        new_tree.add_child_sorted(&parent_id, &id);
                    } else {
                        new_tree.root_ids.push(id.clone());
                        let mut roots = new_tree.root_ids.clone();
                        Self::sort_children(&mut roots, &new_tree.nodes);
                        new_tree.root_ids = roots;
                    }
                } else {
                    new_tree.root_ids.push(id.clone());
                    let mut roots = new_tree.root_ids.clone();
                    Self::sort_children(&mut roots, &new_tree.nodes);
                    new_tree.root_ids = roots;
                }
            }
            FolderTreeChange::Updated(folder) => {
                let id = folder.id.clone();
                if let Some(existing_node) = new_tree.nodes.get(&id) {
                    let old_parent_id = existing_node.folder.parent_id.clone();
                    let new_parent_id = folder.parent_id.clone();
                    
                    // Update the folder data
                    if let Some(node) = new_tree.nodes.get_mut(&id) {
                        node.folder = folder;
                    }
                    
                    // If parent or sort order changed, we might need to re-sort or move it
                    if old_parent_id != new_parent_id {
                        // Remove from old parent
                        if let Some(old_parent) = old_parent_id {
                            new_tree.remove_child(&old_parent, &id);
                        } else {
                            new_tree.root_ids.retain(|child_id| child_id != &id);
                        }

                        // Add to new parent
                        if let Some(new_parent) = new_parent_id {
                            if new_tree.nodes.contains_key(&new_parent) {
                                new_tree.add_child_sorted(&new_parent, &id);
                            } else {
                                new_tree.root_ids.push(id.clone());
                                let mut roots = new_tree.root_ids.clone();
                                Self::sort_children(&mut roots, &new_tree.nodes);
                                new_tree.root_ids = roots;
                            }
                        } else {
                            new_tree.root_ids.push(id.clone());
                            let mut roots = new_tree.root_ids.clone();
                            Self::sort_children(&mut roots, &new_tree.nodes);
                            new_tree.root_ids = roots;
                        }
                    } else {
                        // Just re-sort the current parent's children in case sort_order or name changed
                        if let Some(parent_id) = new_parent_id {
                            new_tree.resort_children(&parent_id);
                        } else {
                            let mut roots = new_tree.root_ids.clone();
                            Self::sort_children(&mut roots, &new_tree.nodes);
                            new_tree.root_ids = roots;
                        }
                    }
                }
            }
            FolderTreeChange::Deleted(id) => {
                if let Some(node) = new_tree.nodes.remove(&id) {
                    // Remove from parent
                    if let Some(parent_id) = node.folder.parent_id {
                        new_tree.remove_child(&parent_id, &id);
                    } else {
                        new_tree.root_ids.retain(|c| c != &id);
                    }
                    
                    // Any children of the deleted node become roots
                    let orphaned_children = node.children;
                    for child_id in &orphaned_children {
                        if let Some(child_node) = new_tree.nodes.get_mut(child_id) {
                            child_node.folder.parent_id = None;
                        }
                    }
                    new_tree.root_ids.extend(orphaned_children);
                    
                    let mut roots = new_tree.root_ids.clone();
                    Self::sort_children(&mut roots, &new_tree.nodes);
                    new_tree.root_ids = roots;
                }
            }
        }

        new_tree
    }

    fn add_child_sorted(&mut self, parent_id: &str, child_id: &str) {
        let mut children = if let Some(node) = self.nodes.get(parent_id) {
            node.children.clone()
        } else {
            return;
        };
        children.push(child_id.to_string());
        Self::sort_children(&mut children, &self.nodes);
        if let Some(node) = self.nodes.get_mut(parent_id) {
            node.children = children;
        }
    }

    fn remove_child(&mut self, parent_id: &str, child_id: &str) {
        if let Some(node) = self.nodes.get_mut(parent_id) {
            node.children.retain(|c| c != child_id);
        }
    }

    fn resort_children(&mut self, parent_id: &str) {
        let mut children = if let Some(node) = self.nodes.get(parent_id) {
            node.children.clone()
        } else {
            return;
        };
        Self::sort_children(&mut children, &self.nodes);
        if let Some(node) = self.nodes.get_mut(parent_id) {
            node.children = children;
        }
    }

    fn sort_children(children: &mut Vec<String>, nodes: &HashMap<String, FolderNode>) {
        children.sort_by(|a, b| {
            if let (Some(node_a), Some(node_b)) = (nodes.get(a), nodes.get(b)) {
                let folder_a = &node_a.folder;
                let folder_b = &node_b.folder;
                folder_a.sort_order.cmp(&folder_b.sort_order)
                    .then_with(|| folder_a.name.cmp(&folder_b.name))
            } else {
                std::cmp::Ordering::Equal
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_folder(id: &str, parent_id: Option<&str>, sort_order: i32) -> Folder {
        Folder {
            id: id.to_string(),
            name: format!("Folder {}", id),
            folder_type: FolderType::Physical,
            parent_id: parent_id.map(|s| s.to_string()),
            library_id: None,
            disk_path: Some(format!("/tmp/{}", id)),
            icon: None,
            color: None,
            sort_order,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_build_tree() {
        let folders = vec![
            make_folder("root1", None, 2),
            make_folder("root2", None, 1),
            make_folder("child1", Some("root1"), 1),
            make_folder("child2", Some("root1"), 2),
            make_folder("grandchild1", Some("child1"), 1),
        ];

        let tree = FolderTree::build(folders);

        // root2 should be before root1 because sort_order=1 vs 2
        assert_eq!(tree.root_ids, vec!["root2", "root1"]);
        
        let root1_node = tree.nodes.get("root1").unwrap();
        assert_eq!(root1_node.children, vec!["child1", "child2"]);
        
        let child1_node = tree.nodes.get("child1").unwrap();
        assert_eq!(child1_node.children, vec!["grandchild1"]);
    }

    #[test]
    fn test_apply_change_add() {
        let mut tree = FolderTree::new();
        let folder = make_folder("new_root", None, 1);
        
        tree = tree.apply_change(FolderTreeChange::Added(folder));
        assert_eq!(tree.root_ids, vec!["new_root"]);

        let child = make_folder("child", Some("new_root"), 1);
        tree = tree.apply_change(FolderTreeChange::Added(child));
        
        assert_eq!(tree.nodes.get("new_root").unwrap().children, vec!["child"]);
    }

    #[test]
    fn test_apply_change_delete() {
        let folders = vec![
            make_folder("root1", None, 1),
            make_folder("child1", Some("root1"), 1),
        ];
        let mut tree = FolderTree::build(folders);

        tree = tree.apply_change(FolderTreeChange::Deleted("child1".to_string()));
        assert!(tree.nodes.get("child1").is_none());
        assert!(tree.nodes.get("root1").unwrap().children.is_empty());
    }

    #[test]
    fn test_apply_change_update_move() {
        let folders = vec![
            make_folder("root1", None, 1),
            make_folder("root2", None, 2),
            make_folder("child1", Some("root1"), 1),
        ];
        let mut tree = FolderTree::build(folders);

        // Move child1 to root2
        let updated_child = make_folder("child1", Some("root2"), 1);
        tree = tree.apply_change(FolderTreeChange::Updated(updated_child));

        assert!(tree.nodes.get("root1").unwrap().children.is_empty());
        assert_eq!(tree.nodes.get("root2").unwrap().children, vec!["child1"]);
    }
}
