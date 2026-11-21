" Syntax highlighting for Graphite graph files (*.graphite).
if exists("b:current_syntax")
  finish
endif

" The block keywords that start a line. `enum`, `input` and `default` also start pure-file lines.
syntax keyword graphiteKeyword graph node edge choose enum input default
" The built-in type of a pure input, and the type after `input NAME :` or `choose NAME :`.
syntax keyword graphiteType bool
" Lookbehind rather than `\zs`: the keyword before the name would otherwise win the match.
syntax match graphiteType "\%(^\s*\%(input\|choose\)\s\+\w\+\s*:\s*\)\@<=\w\+"
" The name of an enum, and its members on the `enum` line or in the indented block below it.
syntax match graphiteEnumName "\%(^\s*enum\s\+\)\@<=\w\+"
syntax match graphiteMember "\%(^\s*enum\s\+\w\+\s\+\)\@<=\w\+\%(\s\+\w\+\)*"
syntax match graphiteMember "^\s\+\zs\%(\%(node\|edge\|choose\|when\|else\|enum\|input\|default\|graph\)\>\)\@!\w\+\ze\%(\s\|$\)"
" The member tested in `when NAME:MEMBER`.
syntax match graphiteMember "\%(\<when\s\+\w\+:\)\@<=\w\+"
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
highlight default link graphiteEnumName Typedef
highlight default link graphiteMember Constant
highlight default link graphiteArrow Operator
" A real colour rather than a link: PreProc is not red in most colour schemes.
" `default` lets a colour scheme override it.
highlight default graphiteDirective ctermfg=Red guifg=#e06c75

let b:current_syntax = "graphite"
