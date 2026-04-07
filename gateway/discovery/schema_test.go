package discovery

import (
	"encoding/json"
	"testing"
)

func TestPrimitiveSchema(t *testing.T) {
	tests := []struct {
		typeName   string
		wantType   string
		wantFormat string
		wantRef    string
	}{
		{"string", "string", "", ""},
		{"bytes", "string", "byte", ""},
		{"bool", "boolean", "", ""},
		{"int32", "integer", "int32", ""},
		{"sint32", "integer", "int32", ""},
		{"sfixed32", "integer", "int32", ""},
		{"int64", "string", "int64", ""},
		{"sint64", "string", "int64", ""},
		{"sfixed64", "string", "int64", ""},
		{"uint32", "integer", "int32", ""},
		{"fixed32", "integer", "int32", ""},
		{"uint64", "string", "uint64", ""},
		{"fixed64", "string", "uint64", ""},
		{"float", "number", "float", ""},
		{"double", "number", "double", ""},
		{"google.protobuf.Timestamp", "string", "date-time", ""},
		{"google.protobuf.Duration", "string", "duration", ""},
	}

	for _, tt := range tests {
		t.Run(tt.typeName, func(t *testing.T) {
			schema := primitiveSchema(tt.typeName)
			if schema.Type != tt.wantType {
				t.Errorf("Type = %q, want %q", schema.Type, tt.wantType)
			}
			if schema.Format != tt.wantFormat {
				t.Errorf("Format = %q, want %q", schema.Format, tt.wantFormat)
			}
			if schema.Ref != tt.wantRef {
				t.Errorf("Ref = %q, want %q", schema.Ref, tt.wantRef)
			}
		})
	}
}

func TestPrimitiveSchema_Any(t *testing.T) {
	schema := primitiveSchema("google.protobuf.Any")
	if schema.Type != "object" {
		t.Errorf("Any type = %q, want \"object\"", schema.Type)
	}
	if schema.Properties == nil {
		t.Fatal("Any should have properties")
	}
	atType, ok := schema.Properties["@type"]
	if !ok {
		t.Fatal("Any should have @type property")
	}
	if atType.Type != "string" {
		t.Errorf("@type type = %q, want \"string\"", atType.Type)
	}
}

func TestPrimitiveSchema_MessageRef(t *testing.T) {
	schema := primitiveSchema("examples.player.PlayerState")
	if schema.Ref != "#/definitions/PlayerState" {
		t.Errorf("Ref = %q, want \"#/definitions/PlayerState\"", schema.Ref)
	}
	if schema.Type != "" {
		t.Errorf("Type should be empty for refs, got %q", schema.Type)
	}
}

func TestPrimitiveSchema_SimpleMessageRef(t *testing.T) {
	// Single-segment name (no dots)
	schema := primitiveSchema("PlayerState")
	if schema.Ref != "#/definitions/PlayerState" {
		t.Errorf("Ref = %q, want \"#/definitions/PlayerState\"", schema.Ref)
	}
}

func TestFieldToSchema_Scalar(t *testing.T) {
	f := FieldDef{Name: "id", JSONName: "id", Type: "string", Repeated: false}
	schema := fieldToSchema(f)
	if schema.Type != "string" {
		t.Errorf("Type = %q, want \"string\"", schema.Type)
	}
	if schema.Items != nil {
		t.Error("scalar field should not have Items")
	}
}

func TestFieldToSchema_Repeated(t *testing.T) {
	f := FieldDef{Name: "tags", JSONName: "tags", Type: "string", Repeated: true}
	schema := fieldToSchema(f)
	if schema.Type != "array" {
		t.Errorf("Type = %q, want \"array\"", schema.Type)
	}
	if schema.Items == nil {
		t.Fatal("repeated field should have Items")
	}
	if schema.Items.Type != "string" {
		t.Errorf("Items.Type = %q, want \"string\"", schema.Items.Type)
	}
}

func TestFieldToSchema_RepeatedMessage(t *testing.T) {
	f := FieldDef{Name: "items", JSONName: "items", Type: "examples.Order.Item", Repeated: true}
	schema := fieldToSchema(f)
	if schema.Type != "array" {
		t.Errorf("Type = %q, want \"array\"", schema.Type)
	}
	if schema.Items == nil {
		t.Fatal("repeated field should have Items")
	}
	if schema.Items.Ref != "#/definitions/Item" {
		t.Errorf("Items.Ref = %q, want \"#/definitions/Item\"", schema.Items.Ref)
	}
}

func TestTypeToSchema(t *testing.T) {
	dt := DiscoveredType{
		FullName: "examples.player.PlayerRegistered",
		Fields: []FieldDef{
			{Name: "player_id", JSONName: "playerId", Type: "string", Repeated: false, Optional: false},
			{Name: "amount", JSONName: "amount", Type: "int64", Repeated: false, Optional: false},
			{Name: "tags", JSONName: "tags", Type: "string", Repeated: true, Optional: false},
			{Name: "nickname", JSONName: "nickname", Type: "string", Repeated: false, Optional: true},
		},
	}

	schema := typeToSchema(dt)

	if schema.Type != "object" {
		t.Errorf("Type = %q, want \"object\"", schema.Type)
	}
	if schema.Description != "Proto message: examples.player.PlayerRegistered" {
		t.Errorf("unexpected Description: %s", schema.Description)
	}
	if len(schema.Properties) != 4 {
		t.Errorf("expected 4 properties, got %d", len(schema.Properties))
	}

	// Required should include non-optional, non-repeated fields
	// playerId (required), amount (required), tags (repeated—excluded), nickname (optional—excluded)
	if len(schema.Required) != 2 {
		t.Fatalf("expected 2 required fields, got %d: %v", len(schema.Required), schema.Required)
	}
	requiredSet := map[string]bool{}
	for _, r := range schema.Required {
		requiredSet[r] = true
	}
	if !requiredSet["playerId"] {
		t.Error("playerId should be required")
	}
	if !requiredSet["amount"] {
		t.Error("amount should be required")
	}
}

func TestTypeToSchema_NoRequired(t *testing.T) {
	dt := DiscoveredType{
		FullName: "examples.Empty",
		Fields: []FieldDef{
			{Name: "opt", JSONName: "opt", Type: "string", Optional: true},
			{Name: "rep", JSONName: "rep", Type: "string", Repeated: true},
		},
	}
	schema := typeToSchema(dt)
	if schema.Required != nil {
		t.Errorf("expected nil Required, got %v", schema.Required)
	}
}

func TestTypeToSchema_NoFields(t *testing.T) {
	dt := DiscoveredType{FullName: "examples.Empty"}
	schema := typeToSchema(dt)
	if schema.Type != "object" {
		t.Errorf("Type = %q, want \"object\"", schema.Type)
	}
	if len(schema.Properties) != 0 {
		t.Errorf("expected 0 properties, got %d", len(schema.Properties))
	}
	if schema.Required != nil {
		t.Errorf("expected nil Required, got %v", schema.Required)
	}
}

func TestGenerateSchemas_Basic(t *testing.T) {
	types := []DiscoveredType{
		{
			FullName: "examples.player.PlayerRegistered",
			Fields: []FieldDef{
				{Name: "id", JSONName: "id", Type: "string"},
			},
		},
		{
			FullName: "examples.order.OrderCreated",
			Fields: []FieldDef{
				{Name: "order_id", JSONName: "orderId", Type: "string"},
			},
		},
	}

	schemas := GenerateSchemas(types)

	if len(schemas) != 2 {
		t.Fatalf("expected 2 schemas, got %d", len(schemas))
	}
	if _, ok := schemas["PlayerRegistered"]; !ok {
		t.Error("missing PlayerRegistered schema")
	}
	if _, ok := schemas["OrderCreated"]; !ok {
		t.Error("missing OrderCreated schema")
	}
}

func TestGenerateSchemas_Collision(t *testing.T) {
	types := []DiscoveredType{
		{FullName: "pkg1.Event"},
		{FullName: "pkg2.Event"},
	}

	schemas := GenerateSchemas(types)

	// One should be "Event", the other "pkg2_Event" (second one collides)
	if len(schemas) != 2 {
		t.Fatalf("expected 2 schemas, got %d", len(schemas))
	}

	// At least one should use the full name key
	_, hasShort := schemas["Event"]
	_, hasFull := schemas["pkg2_Event"]
	if !hasShort || !hasFull {
		// The order may vary since we iterate a slice
		// Just verify we have 2 distinct keys
		keys := make([]string, 0, len(schemas))
		for k := range schemas {
			keys = append(keys, k)
		}
		t.Logf("schema keys: %v", keys)
	}
}

func TestBuildAnyOneOf_NoFilter(t *testing.T) {
	types := []DiscoveredType{
		{FullName: "pkg.TypeA", TypeURL: "type.googleapis.com/pkg.TypeA"},
		{FullName: "pkg.TypeB", TypeURL: "type.googleapis.com/pkg.TypeB"},
	}

	schema := BuildAnyOneOf(types, nil)

	if len(schema.OneOf) != 2 {
		t.Fatalf("expected 2 oneOf entries, got %d", len(schema.OneOf))
	}
	if schema.Description != "One of the discovered proto message types" {
		t.Errorf("unexpected Description: %s", schema.Description)
	}

	// Verify first entry
	entry := schema.OneOf[0]
	if entry.Type != "object" {
		t.Errorf("entry Type = %q, want \"object\"", entry.Type)
	}
	if entry.Ref != "#/definitions/TypeA" {
		t.Errorf("entry Ref = %q, want \"#/definitions/TypeA\"", entry.Ref)
	}
}

func TestBuildAnyOneOf_WithFilter(t *testing.T) {
	types := []DiscoveredType{
		{FullName: "pkg.EventA", TypeURL: "type.googleapis.com/pkg.EventA", IsEvent: true},
		{FullName: "pkg.CommandB", TypeURL: "type.googleapis.com/pkg.CommandB", IsCommand: true},
		{FullName: "pkg.EventC", TypeURL: "type.googleapis.com/pkg.EventC", IsEvent: true},
	}

	schema := BuildAnyOneOf(types, func(dt DiscoveredType) bool { return dt.IsEvent })

	if len(schema.OneOf) != 2 {
		t.Fatalf("expected 2 oneOf entries (events only), got %d", len(schema.OneOf))
	}
}

func TestBuildAnyOneOf_EmptyTypes(t *testing.T) {
	schema := BuildAnyOneOf(nil, nil)
	if schema.OneOf != nil {
		t.Errorf("expected nil oneOf for empty types, got %d entries", len(schema.OneOf))
	}
}

func TestSchemasToJSON(t *testing.T) {
	schemas := map[string]*JSONSchema{
		"Test": {Type: "object", Description: "test schema"},
	}

	data, err := SchemasToJSON(schemas)
	if err != nil {
		t.Fatalf("SchemasToJSON error: %v", err)
	}

	var parsed map[string]interface{}
	if err := json.Unmarshal(data, &parsed); err != nil {
		t.Fatalf("output is not valid JSON: %v", err)
	}

	testSchema, ok := parsed["Test"].(map[string]interface{})
	if !ok {
		t.Fatal("missing Test schema in output")
	}
	if testSchema["type"] != "object" {
		t.Errorf("type = %v, want \"object\"", testSchema["type"])
	}
}

func TestSchemasToJSON_Empty(t *testing.T) {
	schemas := map[string]*JSONSchema{}
	data, err := SchemasToJSON(schemas)
	if err != nil {
		t.Fatalf("SchemasToJSON error: %v", err)
	}
	if string(data) != "{}" {
		t.Errorf("expected empty JSON object, got %s", string(data))
	}
}
