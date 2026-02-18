// angzarr-gen generates router construction code from OO-style comment annotations.
//
// Usage:
//
//	go generate ./...
//
// In your aggregate file:
//
//	//go:generate angzarr-gen aggregate --type=PlayerAggregate
//
//	type PlayerAggregate struct{}
//
//	// @handler RegisterPlayer
//	func (a *PlayerAggregate) Register(cb *pb.CommandBook, cmd *pb.RegisterPlayer, state *PlayerState, seq uint32) (*pb.EventBook, error) { ... }
//
//	// @rejected domain=payment command=ProcessPayment
//	func (a *PlayerAggregate) HandlePaymentRejected(notification *pb.Notification, state *PlayerState) (*pb.BusinessResponse, error) { ... }
//
// This generates player_aggregate_gen.go with:
//
//	func NewPlayerAggregateRouter(agg *PlayerAggregate) *angzarr.CommandRouter[PlayerState] {
//	    return angzarr.NewCommandRouter("player", agg.RebuildState).
//	        On("RegisterPlayer", wrapRegister(agg)).
//	        OnRejected("payment", "ProcessPayment", wrapHandlePaymentRejected(agg))
//	}
package main

import (
	"bytes"
	"flag"
	"fmt"
	"go/ast"
	"go/format"
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"text/template"
)

// Marker patterns
var (
	handlerPattern  = regexp.MustCompile(`@handler\s+(\w+)`)
	reactsPattern   = regexp.MustCompile(`@reacts\s+(\w+)(?:\s+domain=(\w+))?`)
	preparesPattern = regexp.MustCompile(`@prepares\s+(\w+)`)
	rejectedPattern = regexp.MustCompile(`@rejected\s+domain=(\w+)\s+command=(\w+)`)
	projectsPattern = regexp.MustCompile(`@projects\s+(\w+)`)
)

// ComponentType represents the type of component being generated
type ComponentType string

const (
	Aggregate      ComponentType = "aggregate"
	Saga           ComponentType = "saga"
	ProcessManager ComponentType = "pm"
	Projector      ComponentType = "projector"
)

// HandlerInfo stores information about a handler method
type HandlerInfo struct {
	MethodName  string
	CommandType string // For @handler
	EventType   string // For @reacts, @prepares, @projects
	Domain      string // For @reacts with domain, @rejected
	Command     string // For @rejected
	IsPrepare   bool
	IsRejected  bool
}

// TypeInfo stores information about the target type
type TypeInfo struct {
	TypeName     string
	Package      string
	StateType    string
	Domain       string // Derived from struct or explicit
	Handlers     []HandlerInfo
	InputDomains map[string][]string // domain -> event types
}

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "Usage: angzarr-gen <component> [flags]")
		fmt.Fprintln(os.Stderr, "Components: aggregate, saga, pm, projector")
		os.Exit(1)
	}

	component := ComponentType(os.Args[1])

	fs := flag.NewFlagSet(string(component), flag.ExitOnError)
	typeName := fs.String("type", "", "Type name to generate for")
	domain := fs.String("domain", "", "Domain name (optional, defaults to lowercase type name)")
	stateType := fs.String("state", "", "State type name (optional)")
	output := fs.String("output", "", "Output file (optional, defaults to <type>_gen.go)")

	if err := fs.Parse(os.Args[2:]); err != nil {
		fmt.Fprintf(os.Stderr, "Error parsing flags: %v\n", err)
		os.Exit(1)
	}

	if *typeName == "" {
		fmt.Fprintln(os.Stderr, "Error: --type is required")
		os.Exit(1)
	}

	// Find the source file in current directory
	dir := "."
	if envDir := os.Getenv("GOFILE"); envDir != "" {
		dir = filepath.Dir(envDir)
	}

	info, err := parseType(dir, *typeName, component)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error parsing type: %v\n", err)
		os.Exit(1)
	}

	if *domain != "" {
		info.Domain = *domain
	}
	if *stateType != "" {
		info.StateType = *stateType
	}

	// Generate output
	code, err := generate(info, component)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error generating code: %v\n", err)
		os.Exit(1)
	}

	// Write output
	outputFile := *output
	if outputFile == "" {
		outputFile = strings.ToLower(*typeName) + "_gen.go"
	}

	if err := os.WriteFile(outputFile, code, 0644); err != nil {
		fmt.Fprintf(os.Stderr, "Error writing output: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("Generated %s\n", outputFile)
}

func parseType(dir, typeName string, component ComponentType) (*TypeInfo, error) {
	fset := token.NewFileSet()
	pkgs, err := parser.ParseDir(fset, dir, nil, parser.ParseComments)
	if err != nil {
		return nil, fmt.Errorf("parsing directory: %w", err)
	}

	info := &TypeInfo{
		TypeName:     typeName,
		InputDomains: make(map[string][]string),
	}

	for pkgName, pkg := range pkgs {
		info.Package = pkgName
		for _, file := range pkg.Files {
			if err := parseFile(file, info, component); err != nil {
				return nil, err
			}
		}
	}

	// Derive domain from type name if not set
	if info.Domain == "" {
		// Remove common suffixes
		name := typeName
		for _, suffix := range []string{"Aggregate", "Saga", "PM", "ProcessManager", "Projector"} {
			name = strings.TrimSuffix(name, suffix)
		}
		info.Domain = strings.ToLower(name)
	}

	return info, nil
}

func parseFile(file *ast.File, info *TypeInfo, component ComponentType) error {
	for _, decl := range file.Decls {
		fn, ok := decl.(*ast.FuncDecl)
		if !ok || fn.Recv == nil {
			continue
		}

		// Check if this is a method on our target type
		recvType := getReceiverType(fn.Recv)
		if recvType != info.TypeName {
			continue
		}

		// Check for comment annotations
		if fn.Doc == nil {
			continue
		}

		for _, comment := range fn.Doc.List {
			text := comment.Text

			// @handler CommandType
			if matches := handlerPattern.FindStringSubmatch(text); len(matches) > 1 {
				info.Handlers = append(info.Handlers, HandlerInfo{
					MethodName:  fn.Name.Name,
					CommandType: matches[1],
				})
			}

			// @reacts EventType [domain=X]
			if matches := reactsPattern.FindStringSubmatch(text); len(matches) > 1 {
				h := HandlerInfo{
					MethodName: fn.Name.Name,
					EventType:  matches[1],
				}
				if len(matches) > 2 && matches[2] != "" {
					h.Domain = matches[2]
					if _, ok := info.InputDomains[h.Domain]; !ok {
						info.InputDomains[h.Domain] = []string{}
					}
					info.InputDomains[h.Domain] = append(info.InputDomains[h.Domain], h.EventType)
				}
				info.Handlers = append(info.Handlers, h)
			}

			// @prepares EventType
			if matches := preparesPattern.FindStringSubmatch(text); len(matches) > 1 {
				info.Handlers = append(info.Handlers, HandlerInfo{
					MethodName: fn.Name.Name,
					EventType:  matches[1],
					IsPrepare:  true,
				})
			}

			// @rejected domain=X command=Y
			if matches := rejectedPattern.FindStringSubmatch(text); len(matches) > 2 {
				info.Handlers = append(info.Handlers, HandlerInfo{
					MethodName: fn.Name.Name,
					Domain:     matches[1],
					Command:    matches[2],
					IsRejected: true,
				})
			}

			// @projects EventType
			if matches := projectsPattern.FindStringSubmatch(text); len(matches) > 1 {
				info.Handlers = append(info.Handlers, HandlerInfo{
					MethodName: fn.Name.Name,
					EventType:  matches[1],
				})
			}
		}
	}

	return nil
}

func getReceiverType(recv *ast.FieldList) string {
	if len(recv.List) == 0 {
		return ""
	}
	switch t := recv.List[0].Type.(type) {
	case *ast.StarExpr:
		if ident, ok := t.X.(*ast.Ident); ok {
			return ident.Name
		}
	case *ast.Ident:
		return t.Name
	}
	return ""
}

func generate(info *TypeInfo, component ComponentType) ([]byte, error) {
	var tmpl *template.Template
	var err error

	switch component {
	case Aggregate:
		tmpl, err = template.New("aggregate").Parse(aggregateTemplate)
	case Saga:
		tmpl, err = template.New("saga").Parse(sagaTemplate)
	case ProcessManager:
		tmpl, err = template.New("pm").Parse(pmTemplate)
	case Projector:
		tmpl, err = template.New("projector").Parse(projectorTemplate)
	default:
		return nil, fmt.Errorf("unknown component type: %s", component)
	}

	if err != nil {
		return nil, fmt.Errorf("parsing template: %w", err)
	}

	var buf bytes.Buffer
	if err := tmpl.Execute(&buf, info); err != nil {
		return nil, fmt.Errorf("executing template: %w", err)
	}

	// Format the generated code
	formatted, err := format.Source(buf.Bytes())
	if err != nil {
		// Return unformatted if formatting fails (helps debugging)
		return buf.Bytes(), nil
	}

	return formatted, nil
}

const aggregateTemplate = `// Code generated by angzarr-gen. DO NOT EDIT.

package {{.Package}}

import (
	angzarr "github.com/angzarr/client/go"
	pb "github.com/angzarr/client/go/proto/angzarr"
)

// New{{.TypeName}}Router creates a CommandRouter from the {{.TypeName}}'s annotated methods.
func New{{.TypeName}}Router(agg *{{.TypeName}}) *angzarr.CommandRouter[{{if .StateType}}{{.StateType}}{{else}}any{{end}}] {
	return angzarr.NewCommandRouter("{{.Domain}}", agg.RebuildState){{range .Handlers}}{{if not .IsRejected}}{{if .CommandType}}.
		On("{{.CommandType}}", wrap{{.MethodName}}(agg)){{end}}{{end}}{{end}}{{range .Handlers}}{{if .IsRejected}}.
		OnRejected("{{.Domain}}", "{{.Command}}", wrap{{.MethodName}}(agg)){{end}}{{end}}
}
{{range .Handlers}}{{if not .IsRejected}}{{if .CommandType}}
func wrap{{.MethodName}}(agg *{{$.TypeName}}) angzarr.CommandHandler[{{if $.StateType}}{{$.StateType}}{{else}}any{{end}}] {
	return func(cb *pb.CommandBook, cmd *pb.Any, state *{{if $.StateType}}{{$.StateType}}{{else}}any{{end}}, seq uint32) (*pb.EventBook, error) {
		// Unpack command and call method
		return agg.{{.MethodName}}(cb, cmd, state, seq)
	}
}
{{end}}{{end}}{{end}}{{range .Handlers}}{{if .IsRejected}}
func wrap{{.MethodName}}(agg *{{$.TypeName}}) angzarr.RejectionHandler[{{if $.StateType}}{{$.StateType}}{{else}}any{{end}}] {
	return func(notification *pb.Notification, state *{{if $.StateType}}{{$.StateType}}{{else}}any{{end}}) (*pb.BusinessResponse, error) {
		return agg.{{.MethodName}}(notification, state)
	}
}
{{end}}{{end}}
`

const sagaTemplate = `// Code generated by angzarr-gen. DO NOT EDIT.

package {{.Package}}

import (
	angzarr "github.com/angzarr/client/go"
)

// New{{.TypeName}}Router creates an EventRouter from the {{.TypeName}}'s annotated methods.
func New{{.TypeName}}Router(saga *{{.TypeName}}) *angzarr.EventRouter {
	return angzarr.NewEventRouter("{{.Domain}}", saga.InputDomain()){{range .Handlers}}{{if .IsPrepare}}.
		Prepare("{{.EventType}}", wrap{{.MethodName}}Prepare(saga)){{else}}{{if .EventType}}.
		On("{{.EventType}}", wrap{{.MethodName}}(saga)){{end}}{{end}}{{end}}
}
`

const pmTemplate = `// Code generated by angzarr-gen. DO NOT EDIT.

package {{.Package}}

import (
	angzarr "github.com/angzarr/client/go"
)

// New{{.TypeName}}Handler creates a ProcessManagerHandler from the {{.TypeName}}'s annotated methods.
func New{{.TypeName}}Handler(pm *{{.TypeName}}) *angzarr.ProcessManagerHandler {
	handler := angzarr.NewProcessManagerHandler("{{.Domain}}"){{range $domain, $types := .InputDomains}}.
		ListenTo("{{$domain}}"{{range $types}}, "{{.}}"{{end}}){{end}}
	return handler
}
`

const projectorTemplate = `// Code generated by angzarr-gen. DO NOT EDIT.

package {{.Package}}

import (
	angzarr "github.com/angzarr/client/go"
)

// New{{.TypeName}}Handler creates a ProjectorHandler from the {{.TypeName}}'s annotated methods.
func New{{.TypeName}}Handler(proj *{{.TypeName}}) *angzarr.ProjectorHandler {
	return angzarr.NewProjectorHandler("{{.Domain}}"){{range .Handlers}}{{if .EventType}}.
		On("{{.EventType}}", wrap{{.MethodName}}(proj)){{end}}{{end}}
}
`
