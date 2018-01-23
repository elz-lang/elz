package listener

import (
	"strings"
	"testing"
)

func TestParse(t *testing.T) {
	res := NewParse(`
	let a = 10
	`)
	expected := `
; ModuleID = 'main'
source_filename = "main"

@a = global float 1.000000e+01
`
	if strings.Contains(res, expected) {
		t.Errorf("expected: `%s`\nactual: `%s`", expected, res)
	}
}
