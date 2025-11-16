use cucumber::{given, then, when, World};

#[derive(Debug, Default, World)]
pub struct KernelWorld {
    list_head: Option<angzarr_list::ListHead>,
    list_entries: Vec<angzarr_list::ListHead>,
    rb_root: Option<angzarr_rbtree::RbRoot>,
}

#[given("an empty list")]
fn empty_list(world: &mut KernelWorld) {
    let mut head = angzarr_list::ListHead::new();
    unsafe {
        head.init();
    }
    world.list_head = Some(head);
}

#[when("I add an entry to the list")]
fn add_entry(world: &mut KernelWorld) {
    let mut entry = angzarr_list::ListHead::new();
    unsafe {
        entry.init();
        if let Some(ref mut head) = world.list_head {
            head.add(&mut entry as *mut angzarr_list::ListHead);
        }
    }
    world.list_entries.push(entry);
}

#[then("the list should not be empty")]
fn list_not_empty(world: &mut KernelWorld) {
    if let Some(ref head) = world.list_head {
        assert!(!head.is_empty());
    }
}

#[given("an empty red-black tree")]
fn empty_rbtree(world: &mut KernelWorld) {
    world.rb_root = Some(angzarr_rbtree::RbRoot::new());
}

#[then("the tree should be empty")]
fn tree_empty(world: &mut KernelWorld) {
    if let Some(ref root) = world.rb_root {
        assert!(root.is_empty());
    }
}

#[tokio::main]
async fn main() {
    KernelWorld::run("angzarr-test-framework/features").await;
}
