package discovery

import (
	"encoding/json"
	"os"
	"sync"
	"testing"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/descriptorpb"
)

// helper to create a temp descriptor file with business types
func createTestDescriptorFile(t *testing.T) string {
	t.Helper()

	stringType := descriptorpb.FieldDescriptorProto_TYPE_STRING
	labelOptional := descriptorpb.FieldDescriptorProto_LABEL_OPTIONAL
	fieldNum := int32(1)

	fds := &descriptorpb.FileDescriptorSet{
		File: []*descriptorpb.FileDescriptorProto{
			{
				Name:    proto.String("player.proto"),
				Package: proto.String("examples.player"),
				Syntax:  proto.String("proto3"),
				MessageType: []*descriptorpb.DescriptorProto{
					{
						Name: proto.String("PlayerRegistered"),
						Field: []*descriptorpb.FieldDescriptorProto{
							{Name: proto.String("player_id"), JsonName: proto.String("playerId"), Number: &fieldNum, Type: &stringType, Label: &labelOptional},
						},
					},
					{
						Name: proto.String("CreatePlayer"),
						Field: []*descriptorpb.FieldDescriptorProto{
							{Name: proto.String("name"), JsonName: proto.String("name"), Number: &fieldNum, Type: &stringType, Label: &labelOptional},
						},
					},
				},
			},
		},
	}

	data, err := proto.Marshal(fds)
	if err != nil {
		t.Fatal(err)
	}

	tmp, err := os.CreateTemp("", "test-svc-descriptor-*.bin")
	if err != nil {
		t.Fatal(err)
	}

	tmp.Write(data)
	tmp.Close()

	t.Cleanup(func() { os.Remove(tmp.Name()) })
	return tmp.Name()
}

func validBaseSpec() []byte {
	return []byte(`{
		"swagger": "2.0",
		"info": {"title": "Test API", "description": "Base"},
		"paths": {},
		"definitions": {}
	}`)
}

func TestNewService_WithDescriptorFile(t *testing.T) {
	descriptorPath := createTestDescriptorFile(t)

	svc, err := NewService(validBaseSpec(), descriptorPath)
	if err != nil {
		t.Fatalf("NewService error: %v", err)
	}

	types := svc.GetTypes()
	if len(types) != 2 {
		t.Fatalf("expected 2 types, got %d", len(types))
	}

	// Should have patched spec (different from base)
	spec := svc.GetSpec()
	if string(spec) == string(validBaseSpec()) {
		t.Error("spec should have been patched")
	}

	// Verify it's valid JSON
	var parsed map[string]any
	if err := json.Unmarshal(spec, &parsed); err != nil {
		t.Fatalf("patched spec is not valid JSON: %v", err)
	}
}

func TestNewService_NoDescriptor(t *testing.T) {
	// No descriptor path, no env var
	os.Unsetenv(DescriptorPathEnvVar)

	svc, err := NewService(validBaseSpec(), "")
	if err != nil {
		t.Fatalf("NewService error: %v", err)
	}

	// Should return base spec unchanged
	if string(svc.GetSpec()) != string(validBaseSpec()) {
		t.Error("spec should be unchanged when no descriptor")
	}

	if len(svc.GetTypes()) != 0 {
		t.Errorf("expected 0 types, got %d", len(svc.GetTypes()))
	}
}

func TestNewService_InvalidDescriptorPath(t *testing.T) {
	_, err := NewService(validBaseSpec(), "/nonexistent/descriptor.bin")
	if err == nil {
		t.Error("expected error for invalid descriptor path")
	}
}

func TestNewService_InvalidBaseSpec(t *testing.T) {
	descriptorPath := createTestDescriptorFile(t)

	_, err := NewService([]byte("not json"), descriptorPath)
	if err == nil {
		t.Error("expected error for invalid base spec JSON")
	}
}

func TestService_GetCollisions_NoCollisions(t *testing.T) {
	descriptorPath := createTestDescriptorFile(t)

	svc, err := NewService(validBaseSpec(), descriptorPath)
	if err != nil {
		t.Fatalf("NewService error: %v", err)
	}

	collisions := svc.GetCollisions()
	if collisions.HasCollisions {
		t.Error("expected no collisions")
	}
}

func TestService_GetInfo(t *testing.T) {
	descriptorPath := createTestDescriptorFile(t)

	svc, err := NewService(validBaseSpec(), descriptorPath)
	if err != nil {
		t.Fatalf("NewService error: %v", err)
	}

	info := svc.GetInfo()

	typeCount, ok := info["type_count"].(int)
	if !ok {
		t.Fatal("type_count should be int")
	}
	if typeCount != 2 {
		t.Errorf("type_count = %d, want 2", typeCount)
	}

	hasCollisions, ok := info["has_collisions"].(bool)
	if !ok {
		t.Fatal("has_collisions should be bool")
	}
	if hasCollisions {
		t.Error("expected has_collisions = false")
	}

	eventCount, ok := info["event_count"].(int)
	if !ok {
		t.Fatal("event_count should be int")
	}
	if eventCount != 1 {
		t.Errorf("event_count = %d, want 1", eventCount)
	}

	commandCount, ok := info["command_count"].(int)
	if !ok {
		t.Fatal("command_count should be int")
	}
	if commandCount != 1 {
		t.Errorf("command_count = %d, want 1", commandCount)
	}
}

func TestService_GetInfo_Empty(t *testing.T) {
	os.Unsetenv(DescriptorPathEnvVar)

	svc, err := NewService(validBaseSpec(), "")
	if err != nil {
		t.Fatalf("NewService error: %v", err)
	}

	info := svc.GetInfo()
	if info["type_count"].(int) != 0 {
		t.Errorf("type_count = %v, want 0", info["type_count"])
	}
	if info["event_count"].(int) != 0 {
		t.Errorf("event_count = %v, want 0", info["event_count"])
	}
}

func TestService_ConcurrentAccess(t *testing.T) {
	descriptorPath := createTestDescriptorFile(t)

	svc, err := NewService(validBaseSpec(), descriptorPath)
	if err != nil {
		t.Fatalf("NewService error: %v", err)
	}

	// Concurrent reads should be safe
	var wg sync.WaitGroup
	for i := 0; i < 100; i++ {
		wg.Add(4)
		go func() {
			defer wg.Done()
			_ = svc.GetSpec()
		}()
		go func() {
			defer wg.Done()
			_ = svc.GetTypes()
		}()
		go func() {
			defer wg.Done()
			_ = svc.GetCollisions()
		}()
		go func() {
			defer wg.Done()
			_ = svc.GetInfo()
		}()
	}
	wg.Wait()
}

func TestNewService_WithCollisions(t *testing.T) {
	// Create a descriptor file with two types that share the same short name
	stringType := descriptorpb.FieldDescriptorProto_TYPE_STRING
	labelOptional := descriptorpb.FieldDescriptorProto_LABEL_OPTIONAL
	fieldNum := int32(1)

	fds := &descriptorpb.FileDescriptorSet{
		File: []*descriptorpb.FileDescriptorProto{
			{
				Name:    proto.String("pkg1.proto"),
				Package: proto.String("examples.pkg1"),
				Syntax:  proto.String("proto3"),
				MessageType: []*descriptorpb.DescriptorProto{
					{
						Name: proto.String("Event"),
						Field: []*descriptorpb.FieldDescriptorProto{
							{Name: proto.String("id"), JsonName: proto.String("id"), Number: &fieldNum, Type: &stringType, Label: &labelOptional},
						},
					},
				},
			},
			{
				Name:    proto.String("pkg2.proto"),
				Package: proto.String("examples.pkg2"),
				Syntax:  proto.String("proto3"),
				MessageType: []*descriptorpb.DescriptorProto{
					{
						Name: proto.String("Event"),
						Field: []*descriptorpb.FieldDescriptorProto{
							{Name: proto.String("id"), JsonName: proto.String("id"), Number: &fieldNum, Type: &stringType, Label: &labelOptional},
						},
					},
				},
			},
		},
	}

	data, err := proto.Marshal(fds)
	if err != nil {
		t.Fatal(err)
	}

	tmp, err := os.CreateTemp("", "test-collision-descriptor-*.bin")
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(tmp.Name())
	tmp.Write(data)
	tmp.Close()

	svc, err := NewService(validBaseSpec(), tmp.Name())
	if err != nil {
		t.Fatalf("NewService error: %v", err)
	}

	collisions := svc.GetCollisions()
	if !collisions.HasCollisions {
		t.Error("expected collisions")
	}
	if _, ok := collisions.Collisions["Event"]; !ok {
		t.Error("expected collision for 'Event'")
	}

	// GetInfo should include collisions
	info := svc.GetInfo()
	if info["has_collisions"] != true {
		t.Error("info should report has_collisions = true")
	}
	if _, ok := info["collisions"]; !ok {
		t.Error("info should include collisions map when collisions exist")
	}
}

func TestCountTypes(t *testing.T) {
	types := []DiscoveredType{
		{IsEvent: true},
		{IsEvent: true},
		{IsCommand: true},
		{},
	}

	events := countTypes(types, func(dt DiscoveredType) bool { return dt.IsEvent })
	if events != 2 {
		t.Errorf("events = %d, want 2", events)
	}

	commands := countTypes(types, func(dt DiscoveredType) bool { return dt.IsCommand })
	if commands != 1 {
		t.Errorf("commands = %d, want 1", commands)
	}

	all := countTypes(types, func(dt DiscoveredType) bool { return true })
	if all != 4 {
		t.Errorf("all = %d, want 4", all)
	}

	none := countTypes(nil, func(dt DiscoveredType) bool { return true })
	if none != 0 {
		t.Errorf("none = %d, want 0", none)
	}
}
