package ast

type Ast interface{}
type Expr interface{}
type Stat interface{}

type StatList []Stat

type Error struct {
	Msg string
}

type Number struct {
	Val string
}

type VarDefination struct {
	Immutable  bool
	Export     bool
	Name       string
	VarType    string
	Expression Expr
}

type Param struct {
	Name string
	Type string
}
type ParamList []Param

type FnDefination struct {
	Export bool
	Name   string
	Params ParamList
	Body   StatList
}
