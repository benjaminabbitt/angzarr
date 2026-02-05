package logic

import (
	"encoding/hex"
	"testing"

	"angzarr"
	angzarrpb "angzarr/proto/angzarr"

	"angzarr/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

var testRoot = []byte{0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10}

func TestHandleItemAdded_generates_reserve_stock(t *testing.T) {
	evt, err := anypb.New(&examples.ItemAdded{
		ProductId:      "SKU-001",
		Name:           "Widget",
		Quantity:       5,
		UnitPriceCents: 1000,
		NewSubtotal:    5000,
	})
	if err != nil {
		t.Fatalf("failed to marshal ItemAdded: %v", err)
	}

	root := &angzarrpb.UUID{Value: testRoot}
	commands := HandleItemAdded(evt, root, "corr-123")

	if len(commands) != 1 {
		t.Fatalf("expected 1 command, got %d", len(commands))
	}

	cmd := commands[0]
	if cmd.Cover.Domain != TargetDomain {
		t.Errorf("expected domain %q, got %q", TargetDomain, cmd.Cover.Domain)
	}
	if cmd.Cover.CorrelationId != "corr-123" {
		t.Errorf("expected correlation ID %q, got %q", "corr-123", cmd.Cover.CorrelationId)
	}

	var reserve examples.ReserveStock
	if err := cmd.Pages[0].Command.UnmarshalTo(&reserve); err != nil {
		t.Fatalf("failed to unmarshal ReserveStock: %v", err)
	}
	if reserve.Quantity != 5 {
		t.Errorf("expected quantity 5, got %d", reserve.Quantity)
	}
	if reserve.OrderId != hex.EncodeToString(testRoot) {
		t.Errorf("expected order ID %q, got %q", hex.EncodeToString(testRoot), reserve.OrderId)
	}
}

func TestHandleItemAdded_targets_product_root(t *testing.T) {
	evt, err := anypb.New(&examples.ItemAdded{
		ProductId: "SKU-001",
		Quantity:  1,
	})
	if err != nil {
		t.Fatalf("failed to marshal ItemAdded: %v", err)
	}

	root := &angzarrpb.UUID{Value: testRoot}
	commands := HandleItemAdded(evt, root, "corr-123")

	if len(commands) != 1 {
		t.Fatalf("expected 1 command, got %d", len(commands))
	}

	expectedRoot := angzarr.ToProtoUUID(angzarr.InventoryProductRoot("SKU-001"))
	if string(commands[0].Cover.Root.Value) != string(expectedRoot.Value) {
		t.Errorf("expected product root %x, got %x", expectedRoot.Value, commands[0].Cover.Root.Value)
	}
}

func TestHandleItemAdded_nil_root_returns_nil(t *testing.T) {
	evt, err := anypb.New(&examples.ItemAdded{
		ProductId: "SKU-001",
		Quantity:  1,
	})
	if err != nil {
		t.Fatalf("failed to marshal ItemAdded: %v", err)
	}

	commands := HandleItemAdded(evt, nil, "corr-123")
	if commands != nil {
		t.Errorf("expected nil, got %v", commands)
	}
}

func TestHandleItemRemoved_generates_release_reservation(t *testing.T) {
	evt, err := anypb.New(&examples.ItemRemoved{
		ProductId:   "SKU-001",
		Quantity:    5,
		NewSubtotal: 0,
	})
	if err != nil {
		t.Fatalf("failed to marshal ItemRemoved: %v", err)
	}

	root := &angzarrpb.UUID{Value: testRoot}
	commands := HandleItemRemoved(evt, root, "corr-456")

	if len(commands) != 1 {
		t.Fatalf("expected 1 command, got %d", len(commands))
	}

	cmd := commands[0]
	if cmd.Cover.Domain != TargetDomain {
		t.Errorf("expected domain %q, got %q", TargetDomain, cmd.Cover.Domain)
	}
	if cmd.Cover.CorrelationId != "corr-456" {
		t.Errorf("expected correlation ID %q, got %q", "corr-456", cmd.Cover.CorrelationId)
	}

	cmdAny := cmd.Pages[0].Command
	if cmdAny == nil {
		t.Fatal("expected command, got nil")
	}

	var release examples.ReleaseReservation
	if err := cmdAny.UnmarshalTo(&release); err != nil {
		t.Fatalf("failed to unmarshal ReleaseReservation: %v", err)
	}
	if release.OrderId != hex.EncodeToString(testRoot) {
		t.Errorf("expected order ID %q, got %q", hex.EncodeToString(testRoot), release.OrderId)
	}

	expectedRoot := angzarr.ToProtoUUID(angzarr.InventoryProductRoot("SKU-001"))
	if string(cmd.Cover.Root.Value) != string(expectedRoot.Value) {
		t.Errorf("expected product root %x, got %x", expectedRoot.Value, cmd.Cover.Root.Value)
	}
}

func TestHandleItemRemoved_nil_root_returns_nil(t *testing.T) {
	evt, err := anypb.New(&examples.ItemRemoved{
		ProductId: "SKU-001",
	})
	if err != nil {
		t.Fatalf("failed to marshal ItemRemoved: %v", err)
	}

	commands := HandleItemRemoved(evt, nil, "corr-456")
	if commands != nil {
		t.Errorf("expected nil, got %v", commands)
	}
}

func TestHandleQuantityUpdated_generates_release_and_reserve(t *testing.T) {
	evt, err := anypb.New(&examples.QuantityUpdated{
		ProductId:   "SKU-001",
		OldQuantity: 3,
		NewQuantity: 7,
		NewSubtotal: 7000,
	})
	if err != nil {
		t.Fatalf("failed to marshal QuantityUpdated: %v", err)
	}

	root := &angzarrpb.UUID{Value: testRoot}
	commands := HandleQuantityUpdated(evt, root, "corr-789")

	if len(commands) != 2 {
		t.Fatalf("expected 2 commands, got %d", len(commands))
	}

	// First command: ReleaseReservation
	releaseAny := commands[0].Pages[0].Command
	var release examples.ReleaseReservation
	if err := releaseAny.UnmarshalTo(&release); err != nil {
		t.Fatalf("first command should be ReleaseReservation: %v", err)
	}
	if release.OrderId != hex.EncodeToString(testRoot) {
		t.Errorf("expected order ID %q, got %q", hex.EncodeToString(testRoot), release.OrderId)
	}

	// Second command: ReserveStock
	reserveAny := commands[1].Pages[0].Command
	var reserve examples.ReserveStock
	if err := reserveAny.UnmarshalTo(&reserve); err != nil {
		t.Fatalf("second command should be ReserveStock: %v", err)
	}
	if reserve.Quantity != 7 {
		t.Errorf("expected quantity 7, got %d", reserve.Quantity)
	}
	if reserve.OrderId != hex.EncodeToString(testRoot) {
		t.Errorf("expected order ID %q, got %q", hex.EncodeToString(testRoot), reserve.OrderId)
	}

	// Both commands target the same product root
	expectedRoot := angzarr.ToProtoUUID(angzarr.InventoryProductRoot("SKU-001"))
	if string(commands[0].Cover.Root.Value) != string(expectedRoot.Value) {
		t.Errorf("release command root mismatch: expected %x, got %x", expectedRoot.Value, commands[0].Cover.Root.Value)
	}
	if string(commands[1].Cover.Root.Value) != string(expectedRoot.Value) {
		t.Errorf("reserve command root mismatch: expected %x, got %x", expectedRoot.Value, commands[1].Cover.Root.Value)
	}
}

func TestHandleQuantityUpdated_nil_root_returns_nil(t *testing.T) {
	evt, err := anypb.New(&examples.QuantityUpdated{
		ProductId:   "SKU-001",
		NewQuantity: 7,
	})
	if err != nil {
		t.Fatalf("failed to marshal QuantityUpdated: %v", err)
	}

	commands := HandleQuantityUpdated(evt, nil, "corr-789")
	if commands != nil {
		t.Errorf("expected nil, got %v", commands)
	}
}

func TestHandleCartCleared_releases_all_items(t *testing.T) {
	evt, err := anypb.New(&examples.CartCleared{
		NewSubtotal: 0,
		Items: []*examples.CartItem{
			{ProductId: "SKU-001", Name: "Widget", Quantity: 2, UnitPriceCents: 1000},
			{ProductId: "SKU-002", Name: "Gadget", Quantity: 3, UnitPriceCents: 2000},
		},
	})
	if err != nil {
		t.Fatalf("failed to marshal CartCleared: %v", err)
	}

	root := &angzarrpb.UUID{Value: testRoot}
	commands := HandleCartCleared(evt, root, "corr-abc")

	if len(commands) != 2 {
		t.Fatalf("expected 2 commands, got %d", len(commands))
	}

	cartID := hex.EncodeToString(testRoot)

	for i, cmd := range commands {
		if cmd.Cover.Domain != TargetDomain {
			t.Errorf("command %d: expected domain %q, got %q", i, TargetDomain, cmd.Cover.Domain)
		}
		if cmd.Cover.CorrelationId != "corr-abc" {
			t.Errorf("command %d: expected correlation ID %q, got %q", i, "corr-abc", cmd.Cover.CorrelationId)
		}

		var release examples.ReleaseReservation
		if err := cmd.Pages[0].Command.UnmarshalTo(&release); err != nil {
			t.Fatalf("command %d: failed to unmarshal ReleaseReservation: %v", i, err)
		}
		if release.OrderId != cartID {
			t.Errorf("command %d: expected order ID %q, got %q", i, cartID, release.OrderId)
		}
	}

	// Verify each command targets the correct product root
	expectedRoot1 := angzarr.ToProtoUUID(angzarr.InventoryProductRoot("SKU-001"))
	expectedRoot2 := angzarr.ToProtoUUID(angzarr.InventoryProductRoot("SKU-002"))

	if string(commands[0].Cover.Root.Value) != string(expectedRoot1.Value) {
		t.Errorf("first command root: expected %x, got %x", expectedRoot1.Value, commands[0].Cover.Root.Value)
	}
	if string(commands[1].Cover.Root.Value) != string(expectedRoot2.Value) {
		t.Errorf("second command root: expected %x, got %x", expectedRoot2.Value, commands[1].Cover.Root.Value)
	}
}

func TestHandleCartCleared_empty_items_returns_nil(t *testing.T) {
	evt, err := anypb.New(&examples.CartCleared{
		NewSubtotal: 0,
		Items:       []*examples.CartItem{},
	})
	if err != nil {
		t.Fatalf("failed to marshal CartCleared: %v", err)
	}

	root := &angzarrpb.UUID{Value: testRoot}
	commands := HandleCartCleared(evt, root, "corr-abc")

	if len(commands) != 0 {
		t.Errorf("expected 0 commands for empty items, got %d", len(commands))
	}
}

func TestHandleCartCleared_nil_root_returns_nil(t *testing.T) {
	evt, err := anypb.New(&examples.CartCleared{
		Items: []*examples.CartItem{
			{ProductId: "SKU-001"},
		},
	})
	if err != nil {
		t.Fatalf("failed to marshal CartCleared: %v", err)
	}

	commands := HandleCartCleared(evt, nil, "corr-abc")
	if commands != nil {
		t.Errorf("expected nil, got %v", commands)
	}
}

func TestDeterministicProductRoot(t *testing.T) {
	root1 := productRoot("SKU-001")
	root2 := productRoot("SKU-001")
	root3 := productRoot("SKU-002")

	if string(root1.Value) != string(root2.Value) {
		t.Errorf("same product_id should produce same root: %x != %x", root1.Value, root2.Value)
	}
	if string(root1.Value) == string(root3.Value) {
		t.Error("different product_ids should produce different roots")
	}
}

func TestProductRootMatchesIdentityModule(t *testing.T) {
	root := productRoot("SKU-001")
	expected := angzarr.ToProtoUUID(angzarr.InventoryProductRoot("SKU-001"))

	if string(root.Value) != string(expected.Value) {
		t.Errorf("productRoot should match angzarr.InventoryProductRoot: %x != %x", root.Value, expected.Value)
	}
}
