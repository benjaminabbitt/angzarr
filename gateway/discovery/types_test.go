package discovery

import "testing"

func TestIsEventName(t *testing.T) {
	tests := []struct {
		name string
		want bool
	}{
		// Past tense "ed" suffix
		{"PlayerRegistered", true},
		{"OrderCompleted", true},
		{"ItemCreated", true},
		{"AccountUpdated", true},
		{"RecordDeleted", true},
		{"TaskChanged", true},
		{"ProcessFailed", true},
		{"JobStarted", true},
		{"SessionEnded", true},

		// Not events
		{"CreatePlayer", false},
		{"UpdateOrder", false},
		{"Player", false},
		{"", false},

		// Edge: name shorter than or equal to suffix length
		{"ed", false},
		// "Created" matches the "ed" suffix (len > len("ed"))
		{"Created", true},

		// Doesn't end in any recognized suffix
		{"Credential", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := isEventName(tt.name); got != tt.want {
				t.Errorf("isEventName(%q) = %v, want %v", tt.name, got, tt.want)
			}
		})
	}
}

func TestIsEventName_EdgeCases(t *testing.T) {
	// "ed" suffix with name exactly len(suffix)+1
	if !isEventName("Xed") {
		t.Error("isEventName(\"Xed\") should be true (len > len(\"ed\"))")
	}

	// "ed" is the shortest suffix; only names with len <= len("ed") should be false
	if isEventName("ed") {
		t.Error("isEventName(\"ed\") should be false (name not longer than suffix)")
	}
	// Longer suffixes like "Created" still match "ed", so they return true
	// Only truly short strings should return false
	if isEventName("X") {
		t.Error("isEventName(\"X\") should be false")
	}
	if isEventName("") {
		t.Error("isEventName(\"\") should be false")
	}
}

func TestIsCommandName(t *testing.T) {
	tests := []struct {
		name string
		want bool
	}{
		// Command prefixes
		{"CreatePlayer", true},
		{"UpdateOrder", true},
		{"DeleteRecord", true},
		{"StartProcess", true},
		{"StopService", true},
		{"RegisterUser", true},
		{"ExecuteQuery", true},
		{"HandleRequest", true},
		{"ProcessPayment", true},
		{"DepositFunds", true},
		{"WithdrawAmount", true},
		{"ReserveSlot", true},
		{"ReleaseResource", true},

		// Not commands
		{"PlayerRegistered", false},
		{"OrderCompleted", false},
		{"Player", false},
		{"", false},
		{"Get", false}, // not a recognized prefix

		// Exact prefix match (no extra chars) — still true since len >= len(prefix)
		{"Create", true},
		{"Update", true},
		{"Delete", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := isCommandName(tt.name); got != tt.want {
				t.Errorf("isCommandName(%q) = %v, want %v", tt.name, got, tt.want)
			}
		})
	}
}

func TestDiscoveredTypeStructure(t *testing.T) {
	// Verify struct fields are properly populated
	dt := DiscoveredType{
		FullName:  "examples.player.PlayerRegistered",
		TypeURL:   "type.googleapis.com/examples.player.PlayerRegistered",
		IsEvent:   true,
		IsCommand: false,
		Fields: []FieldDef{
			{Name: "player_id", JSONName: "playerId", Type: "string", Repeated: false, Optional: false},
			{Name: "tags", JSONName: "tags", Type: "string", Repeated: true, Optional: false},
			{Name: "nickname", JSONName: "nickname", Type: "string", Repeated: false, Optional: true},
		},
	}

	if dt.FullName != "examples.player.PlayerRegistered" {
		t.Errorf("unexpected FullName: %s", dt.FullName)
	}
	if dt.TypeURL != "type.googleapis.com/examples.player.PlayerRegistered" {
		t.Errorf("unexpected TypeURL: %s", dt.TypeURL)
	}
	if !dt.IsEvent {
		t.Error("expected IsEvent to be true")
	}
	if dt.IsCommand {
		t.Error("expected IsCommand to be false")
	}
	if len(dt.Fields) != 3 {
		t.Errorf("expected 3 fields, got %d", len(dt.Fields))
	}
	if dt.Fields[1].Repeated != true {
		t.Error("expected tags field to be repeated")
	}
	if dt.Fields[2].Optional != true {
		t.Error("expected nickname field to be optional")
	}
}
