use gio::prelude::*;
use vmod::mod_entry::{SectionHeader, ModEntry};
use std::path::PathBuf;

/// Test that we can create sections and get their order
#[test]
fn test_section_order_basics() {
    // Initialize GTK for GObject support
    gtk4::init().ok();

    let section1 = SectionHeader::new("Section 1", 0);
    let section2 = SectionHeader::new("Section 2", 1);

    assert_eq!(section1.order(), 0);
    assert_eq!(section2.order(), 1);

    // Test setting order
    section1.set_order(5);
    assert_eq!(section1.order(), 5);
}

/// Test ListStore with sections and mods - verify order-based sorting
#[test]
fn test_liststore_with_sections_and_mods() {
    gtk4::init().ok();

    let model = gio::ListStore::new::<glib::Object>();

    // Create items: section at 0, mod at 1, mod at 2, section at 3
    let section1 = SectionHeader::new("Section 1", 0);
    let mod1 = ModEntry::new("mod1".to_string(), PathBuf::from("/fake/mod1"), 1);
    let mod2 = ModEntry::new("mod2".to_string(), PathBuf::from("/fake/mod2"), 2);
    let section2 = SectionHeader::new("Section 2", 3);

    model.append(&section1);
    model.append(&mod1);
    model.append(&mod2);
    model.append(&section2);

    assert_eq!(model.n_items(), 4);

    // Verify we can retrieve items and check their types
    let item0 = model.item(0).unwrap();
    let item1 = model.item(1).unwrap();

    assert!(item0.downcast_ref::<SectionHeader>().is_some(), "Item 0 should be a section");
    assert!(item1.downcast_ref::<ModEntry>().is_some(), "Item 1 should be a mod");
}

/// Test swapping order values between section and mod
#[test]
fn test_swap_section_with_mod() {
    gtk4::init().ok();

    let section = SectionHeader::new("Section", 0);
    let mod1 = ModEntry::new("mod1".to_string(), PathBuf::from("/fake/mod1"), 1);

    // Initial state
    assert_eq!(section.order(), 0);
    assert_eq!(mod1.order(), 1);

    // Swap orders (simulating move down for section)
    let section_order = section.order();
    let mod_order = mod1.order();

    section.set_order(mod_order);
    mod1.set_order(section_order);

    // After swap
    assert_eq!(section.order(), 1, "Section should now have order 1");
    assert_eq!(mod1.order(), 0, "Mod should now have order 0");
}

/// Test rebuild_model_sorted logic
#[test]
fn test_rebuild_sorted() {
    gtk4::init().ok();

    let model = gio::ListStore::new::<glib::Object>();

    // Create items with specific orders
    let section1 = SectionHeader::new("Section 1", 2);  // Will be 3rd
    let mod1 = ModEntry::new("mod1".to_string(), PathBuf::from("/fake/mod1"), 0);  // Will be 1st
    let mod2 = ModEntry::new("mod2".to_string(), PathBuf::from("/fake/mod2"), 1);  // Will be 2nd
    let section2 = SectionHeader::new("Section 2", 3);  // Will be 4th

    // Add in wrong order
    model.append(&section1);
    model.append(&mod2);
    model.append(&section2);
    model.append(&mod1);

    // Now rebuild sorted (inline the logic here to test it)
    let mut items: Vec<(u32, u8, glib::Object)> = Vec::new();

    for i in 0..model.n_items() {
        if let Some(item) = model.item(i) {
            if let Some(mod_entry) = item.downcast_ref::<ModEntry>() {
                items.push((mod_entry.order(), 1, item)); // priority 1 for mods
            } else if let Some(section) = item.downcast_ref::<SectionHeader>() {
                items.push((section.order(), 0, item)); // priority 0 for sections
            }
        }
    }

    // Sort by order, then by priority (sections first at same position)
    items.sort_by_key(|(order, priority, _)| (*order, *priority));

    model.remove_all();
    for (_, _, obj) in items {
        model.append(&obj);
    }

    // Verify order: mod1 (0), mod2 (1), section1 (2), section2 (3)
    assert_eq!(model.n_items(), 4);

    let item0 = model.item(0).unwrap();
    let item1 = model.item(1).unwrap();
    let item2 = model.item(2).unwrap();
    let item3 = model.item(3).unwrap();

    // Check types and orders
    let mod_at_0 = item0.downcast_ref::<ModEntry>().expect("Position 0 should be mod1");
    assert_eq!(mod_at_0.order(), 0);

    let mod_at_1 = item1.downcast_ref::<ModEntry>().expect("Position 1 should be mod2");
    assert_eq!(mod_at_1.order(), 1);

    let section_at_2 = item2.downcast_ref::<SectionHeader>().expect("Position 2 should be section1");
    assert_eq!(section_at_2.order(), 2);

    let section_at_3 = item3.downcast_ref::<SectionHeader>().expect("Position 3 should be section2");
    assert_eq!(section_at_3.order(), 3);
}

/// Test moving a section down
#[test]
fn test_move_section_down() {
    gtk4::init().ok();

    let model = gio::ListStore::new::<glib::Object>();

    // Section at position 0, mods at 1, 2, 3
    let section = SectionHeader::new("Section", 0);
    let mod1 = ModEntry::new("mod1".to_string(), PathBuf::from("/fake/mod1"), 1);
    let mod2 = ModEntry::new("mod2".to_string(), PathBuf::from("/fake/mod2"), 2);
    let mod3 = ModEntry::new("mod3".to_string(), PathBuf::from("/fake/mod3"), 3);

    model.append(&section);
    model.append(&mod1);
    model.append(&mod2);
    model.append(&mod3);

    eprintln!("BEFORE move down:");
    for i in 0..model.n_items() {
        let item = model.item(i).unwrap();
        if let Some(s) = item.downcast_ref::<SectionHeader>() {
            eprintln!("  [{}] Section '{}' order={}", i, s.name(), s.order());
        } else if let Some(m) = item.downcast_ref::<ModEntry>() {
            eprintln!("  [{}] Mod '{}' order={}", i, m.name(), m.order());
        }
    }

    // Move section down: swap with mod1
    let position = 0u32;
    let current_item = model.item(position).unwrap();
    let next_item = model.item(position + 1).unwrap();

    // Get order values
    fn get_order(item: &glib::Object) -> u32 {
        if let Some(m) = item.downcast_ref::<ModEntry>() {
            m.order()
        } else if let Some(s) = item.downcast_ref::<SectionHeader>() {
            s.order()
        } else {
            panic!("Unknown item type")
        }
    }

    fn set_order(item: &glib::Object, order: u32) {
        if let Some(m) = item.downcast_ref::<ModEntry>() {
            m.set_order(order);
        } else if let Some(s) = item.downcast_ref::<SectionHeader>() {
            s.set_order(order);
        }
    }

    let current_order = get_order(&current_item);
    let next_order = get_order(&next_item);

    eprintln!("Swapping: current_order={}, next_order={}", current_order, next_order);

    set_order(&current_item, next_order);
    set_order(&next_item, current_order);

    eprintln!("After swap: section.order={}, mod1.order={}", section.order(), mod1.order());

    // Rebuild sorted
    let mut items: Vec<(u32, u8, glib::Object)> = Vec::new();
    for i in 0..model.n_items() {
        if let Some(item) = model.item(i) {
            if let Some(mod_entry) = item.downcast_ref::<ModEntry>() {
                items.push((mod_entry.order(), 1, item));
            } else if let Some(sec) = item.downcast_ref::<SectionHeader>() {
                items.push((sec.order(), 0, item));
            }
        }
    }
    items.sort_by_key(|(order, priority, _)| (*order, *priority));

    model.remove_all();
    for (_, _, obj) in items {
        model.append(&obj);
    }

    eprintln!("AFTER move down:");
    for i in 0..model.n_items() {
        let item = model.item(i).unwrap();
        if let Some(s) = item.downcast_ref::<SectionHeader>() {
            eprintln!("  [{}] Section '{}' order={}", i, s.name(), s.order());
        } else if let Some(m) = item.downcast_ref::<ModEntry>() {
            eprintln!("  [{}] Mod '{}' order={}", i, m.name(), m.order());
        }
    }

    // Verify: mod1 should now be at position 0, section at position 1
    let item0 = model.item(0).unwrap();
    let item1 = model.item(1).unwrap();

    assert!(item0.downcast_ref::<ModEntry>().is_some(), "Position 0 should now be mod1");
    assert!(item1.downcast_ref::<SectionHeader>().is_some(), "Position 1 should now be section");

    // Verify orders
    assert_eq!(get_order(&item0), 0, "mod1 should have order 0");
    assert_eq!(get_order(&item1), 1, "section should have order 1");
}

/// Test moving a section down multiple times
#[test]
fn test_move_section_down_multiple_times() {
    gtk4::init().ok();

    fn get_order(item: &glib::Object) -> u32 {
        if let Some(m) = item.downcast_ref::<ModEntry>() { m.order() }
        else if let Some(s) = item.downcast_ref::<SectionHeader>() { s.order() }
        else { panic!("Unknown item type") }
    }

    fn set_order(item: &glib::Object, order: u32) {
        if let Some(m) = item.downcast_ref::<ModEntry>() { m.set_order(order); }
        else if let Some(s) = item.downcast_ref::<SectionHeader>() { s.set_order(order); }
    }

    fn rebuild_sorted(model: &gio::ListStore) {
        let mut items: Vec<(u32, u8, glib::Object)> = Vec::new();
        for i in 0..model.n_items() {
            if let Some(item) = model.item(i) {
                if let Some(mod_entry) = item.downcast_ref::<ModEntry>() {
                    items.push((mod_entry.order(), 1, item));
                } else if let Some(sec) = item.downcast_ref::<SectionHeader>() {
                    items.push((sec.order(), 0, item));
                }
            }
        }
        items.sort_by_key(|(order, priority, _)| (*order, *priority));
        model.remove_all();
        for (_, _, obj) in items { model.append(&obj); }
    }

    fn move_item_down(model: &gio::ListStore, position: u32) -> bool {
        if position >= model.n_items() - 1 {
            eprintln!("  Cannot move down: already at bottom");
            return false;
        }

        let current_item = model.item(position).unwrap();
        let next_item = model.item(position + 1).unwrap();

        let current_order = get_order(&current_item);
        let next_order = get_order(&next_item);

        eprintln!("  Swapping positions {} (order {}) and {} (order {})",
                  position, current_order, position + 1, next_order);

        set_order(&current_item, next_order);
        set_order(&next_item, current_order);
        rebuild_sorted(model);
        true
    }

    fn print_model(model: &gio::ListStore, label: &str) {
        eprintln!("{}:", label);
        for i in 0..model.n_items() {
            let item = model.item(i).unwrap();
            if let Some(s) = item.downcast_ref::<SectionHeader>() {
                eprintln!("  [{}] Section '{}' order={}", i, s.name(), s.order());
            } else if let Some(m) = item.downcast_ref::<ModEntry>() {
                eprintln!("  [{}] Mod '{}' order={}", i, m.name(), m.order());
            }
        }
    }

    let model = gio::ListStore::new::<glib::Object>();

    // Section at position 0, mods at 1, 2, 3
    let section = SectionHeader::new("Section", 0);
    let mod1 = ModEntry::new("mod1".to_string(), PathBuf::from("/fake/mod1"), 1);
    let mod2 = ModEntry::new("mod2".to_string(), PathBuf::from("/fake/mod2"), 2);
    let mod3 = ModEntry::new("mod3".to_string(), PathBuf::from("/fake/mod3"), 3);

    model.append(&section);
    model.append(&mod1);
    model.append(&mod2);
    model.append(&mod3);

    print_model(&model, "INITIAL");

    // Move section down 3 times (to the bottom)
    eprintln!("\n=== Move 1 ===");
    assert!(move_item_down(&model, 0), "First move should succeed");
    print_model(&model, "After move 1");

    // Find where section is now
    let section_pos = (0..model.n_items())
        .find(|&i| model.item(i).unwrap().downcast_ref::<SectionHeader>().is_some())
        .expect("Section should still exist");
    eprintln!("Section is now at position {}", section_pos);
    assert_eq!(section_pos, 1, "Section should be at position 1 after first move");

    eprintln!("\n=== Move 2 ===");
    assert!(move_item_down(&model, section_pos as u32), "Second move should succeed");
    print_model(&model, "After move 2");

    let section_pos = (0..model.n_items())
        .find(|&i| model.item(i).unwrap().downcast_ref::<SectionHeader>().is_some())
        .expect("Section should still exist");
    eprintln!("Section is now at position {}", section_pos);
    assert_eq!(section_pos, 2, "Section should be at position 2 after second move");

    eprintln!("\n=== Move 3 ===");
    assert!(move_item_down(&model, section_pos as u32), "Third move should succeed");
    print_model(&model, "After move 3");

    let section_pos = (0..model.n_items())
        .find(|&i| model.item(i).unwrap().downcast_ref::<SectionHeader>().is_some())
        .expect("Section should still exist");
    eprintln!("Section is now at position {}", section_pos);
    assert_eq!(section_pos, 3, "Section should be at position 3 (bottom) after third move");
}

/// Test two sections next to each other
#[test]
fn test_two_sections_adjacent() {
    gtk4::init().ok();

    fn get_order(item: &glib::Object) -> u32 {
        if let Some(m) = item.downcast_ref::<ModEntry>() { m.order() }
        else if let Some(s) = item.downcast_ref::<SectionHeader>() { s.order() }
        else { panic!("Unknown item type") }
    }

    fn set_order(item: &glib::Object, order: u32) {
        if let Some(m) = item.downcast_ref::<ModEntry>() { m.set_order(order); }
        else if let Some(s) = item.downcast_ref::<SectionHeader>() { s.set_order(order); }
    }

    fn rebuild_sorted(model: &gio::ListStore) {
        let mut items: Vec<(u32, u8, glib::Object)> = Vec::new();
        for i in 0..model.n_items() {
            if let Some(item) = model.item(i) {
                if let Some(mod_entry) = item.downcast_ref::<ModEntry>() {
                    items.push((mod_entry.order(), 1, item));
                } else if let Some(sec) = item.downcast_ref::<SectionHeader>() {
                    items.push((sec.order(), 0, item));
                }
            }
        }
        items.sort_by_key(|(order, priority, _)| (*order, *priority));
        model.remove_all();
        for (_, _, obj) in items { model.append(&obj); }
    }

    fn move_item_down(model: &gio::ListStore, position: u32) -> bool {
        if position >= model.n_items() - 1 { return false; }

        let current_item = model.item(position).unwrap();
        let next_item = model.item(position + 1).unwrap();

        let current_order = get_order(&current_item);
        let next_order = get_order(&next_item);

        set_order(&current_item, next_order);
        set_order(&next_item, current_order);
        rebuild_sorted(model);
        true
    }

    fn print_model(model: &gio::ListStore, label: &str) {
        eprintln!("{}:", label);
        for i in 0..model.n_items() {
            let item = model.item(i).unwrap();
            if let Some(s) = item.downcast_ref::<SectionHeader>() {
                eprintln!("  [{}] Section '{}' order={}", i, s.name(), s.order());
            } else if let Some(m) = item.downcast_ref::<ModEntry>() {
                eprintln!("  [{}] Mod '{}' order={}", i, m.name(), m.order());
            }
        }
    }

    let model = gio::ListStore::new::<glib::Object>();

    // Two sections at 0 and 1, then mods
    let section1 = SectionHeader::new("Section1", 0);
    let section2 = SectionHeader::new("Section2", 1);
    let mod1 = ModEntry::new("mod1".to_string(), PathBuf::from("/fake/mod1"), 2);
    let mod2 = ModEntry::new("mod2".to_string(), PathBuf::from("/fake/mod2"), 3);

    model.append(&section1);
    model.append(&section2);
    model.append(&mod1);
    model.append(&mod2);

    print_model(&model, "INITIAL - two sections at top");

    // Move section1 down (should swap with section2)
    eprintln!("\n=== Moving Section1 down ===");
    assert!(move_item_down(&model, 0), "Move should succeed");
    print_model(&model, "After moving Section1 down");

    // Verify: section2 should now be at position 0, section1 at position 1
    let item0 = model.item(0).unwrap();
    let item1 = model.item(1).unwrap();

    let sec0 = item0.downcast_ref::<SectionHeader>().expect("Position 0 should be a section");
    let sec1 = item1.downcast_ref::<SectionHeader>().expect("Position 1 should be a section");

    assert_eq!(sec0.name(), "Section2", "Section2 should now be at position 0");
    assert_eq!(sec1.name(), "Section1", "Section1 should now be at position 1");
}
