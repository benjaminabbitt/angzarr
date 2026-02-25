package discovery

import (
	"encoding/json"
	"fmt"
	"strings"
)

// JSONSchema represents a JSON Schema definition.
type JSONSchema struct {
	Type        string                 `json:"type,omitempty"`
	Description string                 `json:"description,omitempty"`
	Properties  map[string]*JSONSchema `json:"properties,omitempty"`
	Items       *JSONSchema            `json:"items,omitempty"`
	Required    []string               `json:"required,omitempty"`
	Ref         string                 `json:"$ref,omitempty"`
	OneOf       []*JSONSchema          `json:"oneOf,omitempty"`
	Format      string                 `json:"format,omitempty"`
}

// GenerateSchemas converts discovered types to JSON schemas.
func GenerateSchemas(types []DiscoveredType) map[string]*JSONSchema {
	schemas := make(map[string]*JSONSchema)

	for _, t := range types {
		schema := typeToSchema(t)
		// Use short name for schema key (last part of full name)
		parts := strings.Split(t.FullName, ".")
		shortName := parts[len(parts)-1]

		// If collision, use full name
		key := shortName
		if _, exists := schemas[key]; exists {
			key = strings.ReplaceAll(t.FullName, ".", "_")
		}
		schemas[key] = schema
	}

	return schemas
}

func typeToSchema(t DiscoveredType) *JSONSchema {
	schema := &JSONSchema{
		Type:        "object",
		Description: fmt.Sprintf("Proto message: %s", t.FullName),
		Properties:  make(map[string]*JSONSchema),
	}

	var required []string

	for _, f := range t.Fields {
		prop := fieldToSchema(f)
		schema.Properties[f.JSONName] = prop

		if !f.Optional && !f.Repeated {
			required = append(required, f.JSONName)
		}
	}

	if len(required) > 0 {
		schema.Required = required
	}

	return schema
}

func fieldToSchema(f FieldDef) *JSONSchema {
	base := primitiveSchema(f.Type)

	if f.Repeated {
		return &JSONSchema{
			Type:  "array",
			Items: base,
		}
	}

	return base
}

func primitiveSchema(typeName string) *JSONSchema {
	switch typeName {
	case "string":
		return &JSONSchema{Type: "string"}
	case "bytes":
		return &JSONSchema{Type: "string", Format: "byte"}
	case "bool":
		return &JSONSchema{Type: "boolean"}
	case "int32", "sint32", "sfixed32":
		return &JSONSchema{Type: "integer", Format: "int32"}
	case "int64", "sint64", "sfixed64":
		return &JSONSchema{Type: "string", Format: "int64"} // JSON doesn't handle 64-bit well
	case "uint32", "fixed32":
		return &JSONSchema{Type: "integer", Format: "int32"}
	case "uint64", "fixed64":
		return &JSONSchema{Type: "string", Format: "uint64"}
	case "float":
		return &JSONSchema{Type: "number", Format: "float"}
	case "double":
		return &JSONSchema{Type: "number", Format: "double"}
	case "google.protobuf.Timestamp":
		return &JSONSchema{Type: "string", Format: "date-time"}
	case "google.protobuf.Duration":
		return &JSONSchema{Type: "string", Format: "duration"}
	case "google.protobuf.Any":
		return &JSONSchema{
			Type:        "object",
			Description: "Any contains an arbitrary serialized protocol buffer message",
			Properties: map[string]*JSONSchema{
				"@type": {Type: "string", Description: "Type URL of the serialized message"},
			},
		}
	default:
		// Reference to another message type
		parts := strings.Split(typeName, ".")
		shortName := parts[len(parts)-1]
		return &JSONSchema{
			Ref: "#/definitions/" + shortName,
		}
	}
}

// BuildAnyOneOf creates a oneOf schema for google.protobuf.Any with discovered types.
func BuildAnyOneOf(types []DiscoveredType, filter func(DiscoveredType) bool) *JSONSchema {
	var oneOf []*JSONSchema

	for _, t := range types {
		if filter != nil && !filter(t) {
			continue
		}

		// Each option includes @type discriminator
		parts := strings.Split(t.FullName, ".")
		shortName := parts[len(parts)-1]

		oneOf = append(oneOf, &JSONSchema{
			Type:        "object",
			Description: t.FullName,
			Properties: map[string]*JSONSchema{
				"@type": {
					Type:  "string",
					Description: fmt.Sprintf("Must be '%s'", t.TypeURL),
				},
			},
			Ref: "#/definitions/" + shortName,
		})
	}

	return &JSONSchema{
		OneOf:       oneOf,
		Description: "One of the discovered proto message types",
	}
}

// SchemasToJSON serializes schemas to JSON bytes.
func SchemasToJSON(schemas map[string]*JSONSchema) ([]byte, error) {
	return json.MarshalIndent(schemas, "", "  ")
}
