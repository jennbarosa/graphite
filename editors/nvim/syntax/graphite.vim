" Syntax highlighting for Graphite graph files (*.graphite).
if exists("b:current_syntax")
  finish
endif

" The block keywords that start a line.
syntax keyword graphiteKeyword graph node edge choose
" The words that follow a node or condition.
syntax keyword graphiteClause when else
" Node flags: a loop, or an end node.
syntax keyword graphiteFlag loop end
" Annotations: @key or @key:value.
syntax match graphiteAnnotation "@[^[:space:]]\+"
" The arrow between nodes and to a choose arm's tag.
syntax match graphiteArrow "->"

highlight default link graphiteKeyword Keyword
highlight default link graphiteClause Conditional
highlight default link graphiteFlag Special
highlight default link graphiteAnnotation Type
highlight default link graphiteArrow Operator

let b:current_syntax = "graphite"
