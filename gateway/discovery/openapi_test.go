package discovery

import (
	"encoding/json"
	"testing"
)

func TestPatchOpenAPISpec_Basic(t *testing.T) {
	baseSpec := []byte(`{
		"swagger": "2.0",
		"info": {"title": "Test API", "description": "Base description"},
		"paths": {},
		"definitions": {"Existing": {"type": "object"}}
	}`)

	types := []DiscoveredType{
		{
			FullName:  "examples.player.PlayerRegistered",
			TypeURL:   "type.googleapis.com/examples.player.PlayerRegistered",
			IsEvent:   true,
			IsCommand: false,
			Fields: []FieldDef{
				{Name: "player_id", JSONName: "playerId", Type: "string"},
			},
		},
	}

	result, err := PatchOpenAPISpec(baseSpec, types)
	if err != nil {
		t.Fatalf("PatchOpenAPISpec error: %v", err)
	}

	var spec map[string]any
	if err := json.Unmarshal(result, &spec); err != nil {
		t.Fatalf("result is not valid JSON: %v", err)
	}

	defs, ok := spec["definitions"].(map[string]any)
	if !ok {
		t.Fatal("missing definitions")
	}

	// Should preserve existing definitions
	if _, ok := defs["Existing"]; !ok {
		t.Error("existing definition should be preserved")
	}

	// Should add discovered type with prefix
	if _, ok := defs["discovered.PlayerRegistered"]; !ok {
		t.Error("missing discovered.PlayerRegistered definition")
	}

	// Should add AnyEvent since we have an event type
	if _, ok := defs["discovered.AnyEvent"]; !ok {
		t.Error("missing discovered.AnyEvent composite definition")
	}

	// Should NOT add AnyCommand since no command types
	if _, ok := defs["discovered.AnyCommand"]; ok {
		t.Error("discovered.AnyCommand should not exist (no command types)")
	}

	// Should patch info description
	info := spec["info"].(map[string]any)
	desc := info["description"].(string)
	if desc == "Base description" {
		t.Error("description should have been patched with discovery note")
	}
}

func TestPatchOpenAPISpec_WithCommands(t *testing.T) {
	baseSpec := []byte(`{
		"swagger": "2.0",
		"info": {"title": "Test"},
		"paths": {},
		"definitions": {}
	}`)

	types := []DiscoveredType{
		{FullName: "pkg.CreatePlayer", TypeURL: "type.googleapis.com/pkg.CreatePlayer", IsCommand: true},
		{FullName: "pkg.PlayerCreated", TypeURL: "type.googleapis.com/pkg.PlayerCreated", IsEvent: true},
	}

	result, err := PatchOpenAPISpec(baseSpec, types)
	if err != nil {
		t.Fatalf("PatchOpenAPISpec error: %v", err)
	}

	var spec map[string]any
	json.Unmarshal(result, &spec)
	defs := spec["definitions"].(map[string]any)

	if _, ok := defs["discovered.AnyEvent"]; !ok {
		t.Error("missing discovered.AnyEvent")
	}
	if _, ok := defs["discovered.AnyCommand"]; !ok {
		t.Error("missing discovered.AnyCommand")
	}
}

func TestPatchOpenAPISpec_NoDefinitionsKey(t *testing.T) {
	// Spec without definitions key — should be created
	baseSpec := []byte(`{
		"swagger": "2.0",
		"info": {"title": "Test"},
		"paths": {}
	}`)

	types := []DiscoveredType{
		{FullName: "pkg.Foo", TypeURL: "type.googleapis.com/pkg.Foo"},
	}

	result, err := PatchOpenAPISpec(baseSpec, types)
	if err != nil {
		t.Fatalf("PatchOpenAPISpec error: %v", err)
	}

	var spec map[string]any
	json.Unmarshal(result, &spec)
	defs, ok := spec["definitions"].(map[string]any)
	if !ok {
		t.Fatal("definitions should have been created")
	}
	if _, ok := defs["discovered.Foo"]; !ok {
		t.Error("missing discovered.Foo")
	}
}

func TestPatchOpenAPISpec_InvalidJSON(t *testing.T) {
	_, err := PatchOpenAPISpec([]byte("not json"), nil)
	if err == nil {
		t.Error("expected error for invalid JSON")
	}
}

func TestPatchOpenAPISpec_EmptyTypes(t *testing.T) {
	baseSpec := []byte(`{
		"swagger": "2.0",
		"info": {"title": "Test"},
		"paths": {},
		"definitions": {"Existing": {"type": "object"}}
	}`)

	result, err := PatchOpenAPISpec(baseSpec, nil)
	if err != nil {
		t.Fatalf("PatchOpenAPISpec error: %v", err)
	}

	var spec map[string]any
	json.Unmarshal(result, &spec)
	defs := spec["definitions"].(map[string]any)

	// Existing preserved, no composites added
	if _, ok := defs["Existing"]; !ok {
		t.Error("existing definition should be preserved")
	}
	if _, ok := defs["discovered.AnyEvent"]; ok {
		t.Error("should not have AnyEvent with no types")
	}
}

func TestFilterTypes(t *testing.T) {
	types := []DiscoveredType{
		{FullName: "A", IsEvent: true},
		{FullName: "B", IsCommand: true},
		{FullName: "C", IsEvent: true},
		{FullName: "D"},
	}

	events := filterTypes(types, func(dt DiscoveredType) bool { return dt.IsEvent })
	if len(events) != 2 {
		t.Errorf("expected 2 events, got %d", len(events))
	}

	commands := filterTypes(types, func(dt DiscoveredType) bool { return dt.IsCommand })
	if len(commands) != 1 {
		t.Errorf("expected 1 command, got %d", len(commands))
	}

	all := filterTypes(types, func(dt DiscoveredType) bool { return true })
	if len(all) != 4 {
		t.Errorf("expected 4, got %d", len(all))
	}

	none := filterTypes(types, func(dt DiscoveredType) bool { return false })
	if len(none) != 0 {
		t.Errorf("expected 0, got %d", len(none))
	}
}

func TestFilterTypes_Nil(t *testing.T) {
	result := filterTypes(nil, func(dt DiscoveredType) bool { return true })
	if result != nil {
		t.Errorf("expected nil for nil input, got %v", result)
	}
}

func TestBuildOneOfDefinition(t *testing.T) {
	types := []DiscoveredType{
		{FullName: "pkg.EventA", TypeURL: "type.googleapis.com/pkg.EventA"},
		{FullName: "pkg.EventB", TypeURL: "type.googleapis.com/pkg.EventB"},
	}

	def := buildOneOfDefinition(types, "Test events")

	desc, ok := def["description"].(string)
	if !ok || desc != "Test events" {
		t.Errorf("description = %v, want \"Test events\"", def["description"])
	}

	oneOf, ok := def["oneOf"].([]map[string]any)
	if !ok {
		t.Fatal("missing oneOf array")
	}
	if len(oneOf) != 2 {
		t.Fatalf("expected 2 oneOf entries, got %d", len(oneOf))
	}

	// Check first entry
	entry := oneOf[0]
	if entry["type"] != "object" {
		t.Errorf("entry type = %v, want \"object\"", entry["type"])
	}
	if entry["description"] != "pkg.EventA" {
		t.Errorf("entry description = %v, want \"pkg.EventA\"", entry["description"])
	}
	if entry["additionalProperties"] != true {
		t.Error("expected additionalProperties: true")
	}

	// Check @type property
	props := entry["properties"].(map[string]any)
	atType := props["@type"].(map[string]any)
	if atType["type"] != "string" {
		t.Errorf("@type type = %v, want \"string\"", atType["type"])
	}
	enumSlice := atType["enum"].([]string)
	if len(enumSlice) != 1 || enumSlice[0] != "type.googleapis.com/pkg.EventA" {
		t.Errorf("@type enum = %v", enumSlice)
	}
}

func TestGetDiscoveryInfo(t *testing.T) {
	types := []DiscoveredType{
		{FullName: "pkg.PlayerCreated", TypeURL: "type.googleapis.com/pkg.PlayerCreated", IsEvent: true},
		{FullName: "pkg.CreatePlayer", TypeURL: "type.googleapis.com/pkg.CreatePlayer", IsCommand: true},
		{FullName: "pkg.PlayerState", TypeURL: "type.googleapis.com/pkg.PlayerState"},
	}

	info := GetDiscoveryInfo(types)

	if info.TypeCount != 3 {
		t.Errorf("TypeCount = %d, want 3", info.TypeCount)
	}
	if info.EventCount != 1 {
		t.Errorf("EventCount = %d, want 1", info.EventCount)
	}
	if info.CommandCount != 1 {
		t.Errorf("CommandCount = %d, want 1", info.CommandCount)
	}
	if len(info.TypeURLs) != 3 {
		t.Errorf("TypeURLs length = %d, want 3", len(info.TypeURLs))
	}
}

func TestGetDiscoveryInfo_Empty(t *testing.T) {
	info := GetDiscoveryInfo(nil)
	if info.TypeCount != 0 {
		t.Errorf("TypeCount = %d, want 0", info.TypeCount)
	}
	if info.EventCount != 0 {
		t.Errorf("EventCount = %d, want 0", info.EventCount)
	}
	if info.CommandCount != 0 {
		t.Errorf("CommandCount = %d, want 0", info.CommandCount)
	}
}

func TestGetDiscoveryInfo_BothEventAndCommand(t *testing.T) {
	// A type that matches both event and command heuristics
	types := []DiscoveredType{
		{FullName: "pkg.Started", TypeURL: "type.googleapis.com/pkg.Started", IsEvent: true, IsCommand: true},
	}

	info := GetDiscoveryInfo(types)
	if info.EventCount != 1 {
		t.Errorf("EventCount = %d, want 1", info.EventCount)
	}
	if info.CommandCount != 1 {
		t.Errorf("CommandCount = %d, want 1", info.CommandCount)
	}
}
