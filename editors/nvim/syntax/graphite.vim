" Syntax highlighting for Graphite graph files (*.graphite).
if exists("b:current_syntax")
  finish
endif

" The block keywords that start a line. `enum` and `input` are pure-file lines.
syntax keyword graphiteKeyword graph node edge choose enum input
" The built-in type of a pure input.
syntax keyword graphiteType bool
" The words that follow a node or condition.
syntax keyword graphiteClause when else
" Node flags: a loop, or an end node.
syntax keyword graphiteFlag loop end
" Annotations: @key or @key:value.
syntax match graphiteAnnotation "@[^[:space:]]\+"
" Directives: @!pure and @!frontend:KEY, at the top of a file. This rule comes after the
" annotation rule, so it wins at the same position.
syntax match graphiteDirective "^@![^[:space:]]\+"
" The arrow between nodes and to a choose arm's tag.
syntax match graphiteArrow "->"

highlight default link graphiteKeyword Keyword
highlight default link graphiteClause Conditional
highlight default link graphiteFlag Special
highlight default link graphiteAnnotation Type
highlight default link graphiteType Type
highlight default link graphiteArrow Operator
" A real colour rather than a link: PreProc is not red in most colour schemes.
" `default` lets a colour scheme override it.
highlight default graphiteDirective ctermfg=Red guifg=#e06c75

let b:current_syntax = "graphite"
