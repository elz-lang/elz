package lexer

import (
	"strings"
	"testing"
)

func TestLexer(t *testing.T) {
	var results []Item
	lexer := Lex("lex", "+123 - name -12.3 世界 + \"string \\\\\"\n let mut fn")
	for item := lexer.NextItem(); item.Type != ItemEOF; item = lexer.NextItem() {
		results = append(results, item)
	}
	if strings.Compare(lexer.name, "lex") != 0 {
		t.Error("Lexer name is wrong!")
	}
	expected := []Item{
		Item{ItemNumber, 0, "+123"},
		Item{ItemMinus, 0, "-"},
		Item{ItemIdent, 0, "name"},
		Item{ItemNumber, 0, "-12.3"},
		Item{ItemIdent, 0, "世界"},
		Item{ItemPlus, 0, "+"},
		Item{ItemString, 0, "\"string \\\\\""},
		Item{ItemKwLet, 0, "let"},
		Item{ItemKwMut, 0, "mut"},
		Item{ItemKwFn, 0, "fn"},
	}
	for i, result := range results {
		if result.Type != expected[i].Type {
			t.Errorf("Token %d, expected: %v, actual: %v", i+1, expected[i].Type, result.Type)
		}
		if result.Val != expected[i].Val {
			t.Errorf("Token %d, expected: %s, actual: %s", i+1, expected[i].Val, result.Val)
		}
	}
}
