graph order
  node placed   @state:placed
  node paid     @state:paid
  node packed   @state:packed
  node shipped  @state:shipped
  node refunded end @state:refunded

  edge placed -> paid     when payment
  edge paid   -> packed   when stock
  edge packed -> shipped  when carrier
  edge shipped -> refunded when returned
  edge placed -> refunded when cancelled
