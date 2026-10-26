{a;b;hidden}.
% The double negation keeps the constraint on the stream: pair capture would
% move :-a,b. into the core, which would then never propose {a,b}.
:-a,not not b.
#minimize{2@2,k:a;-3@1,r:b}. #show a/0. #show b/0. #show selected:b.
