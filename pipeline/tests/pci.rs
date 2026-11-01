//! Tests for finding PCI IDs for the hardware table's GPUs in pci.ids, so a
//! detected GPU can be identified exactly.

use rigcheck_core::Catalog;
use rigcheck_pipeline::pci::{PciId, ids};

const PCI_IDS: &str = include_str!("../../data/fixtures/pci/pci.ids");

#[test]
fn finds_ids_for_table_gpus_by_their_bracketed_name() {
    let catalog = Catalog::from_toml(include_str!("../../data/hardware.toml")).unwrap();

    let found = ids(&catalog, PCI_IDS);

    let id = |vendor_id, device_id, name: &str| PciId {
        vendor_id,
        device_id,
        name: name.to_owned(),
    };
    assert_eq!(
        found,
        [
            // "Navi 23 [Radeon RX 6600/6600 XT/6600M]": only RX 6600 is in the table.
            id(0x1002, 0x73ff, "Radeon RX 6600"),
            id(0x10de, 0x13c2, "GeForce GTX 970"),
            id(0x10de, 0x1c03, "GeForce GTX 1060 6GB"),
            id(0x8086, 0x56a5, "Arc A380"),
        ],
        "skipped: Barcelo (no name), Ellesmere (5 cards share it), RTX 2060 Rev. A (unknown)"
    );
}
