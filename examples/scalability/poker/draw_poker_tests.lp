%%------------------
#include "draw_poker.lp".
%%------------------

% phase(five_card_hero_hand).

% Models       : 2598960
% Calls        : 1
% Time         : 1.939s (Solving: 1.93s 1st Model: 0.00s Unsat: 0.00s)
% CPU Time     : 19.342s
% Threads      : 16       (Winner: 15)

% phase(hero_hand).

% Models       : 133784560
% Calls        : 1
% Time         : 102.933s (Solving: 102.93s 1st Model: 0.00s Unsat: 0.00s)
% CPU Time     : 1038.515s
% Threads      : 16       (Winner: 15)

% clingo draw_poker_tests.lp 0 --project=project --quiet=1,0

player(hero).
% phase(hero_hand).
phase(five_card_hero_hand).

% Models       : 2598960
% Calls        : 1
% Time         : 32.894s (Solving: 32.77s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 506.484s
% Threads      : 16       (Winner: 3)

% Models       : 2598960
% Calls        : 1
% Time         : 94.402s (Solving: 94.21s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 745.592s

% Models       : 1514 % trendy
% Calls        : 1
% Time         : 16.296s (Solving: 15.54s 1st Model: 0.02s Unsat: 0.01s)
% CPU Time     : 123.584s

%%                                      Freq.     Distinct
% :- not top_hand(hero, 10, _, _, _). % 4       |   1    [1] - rf
:- not top_hand(hero,  9, _, _, _). % 36      |   9    [9] - sf
% :- not top_hand(hero,  8, _, _, _). % 624     | 156  [156] - 4k
% :- not top_hand(hero,  7, _, _, _). % 3744    | 156  [156] - fh
% :- not top_hand(hero,  6, _, _, _). % 5108    |   8 [1277] - fl
% :- not top_hand(hero,  5, _, _, _). % 10200   |  10   [10] - st
% :- not top_hand(hero,  4, _, _, _). % 54912   | 143  [858] - 3k
% :- not top_hand(hero,  3, _, _, _). % 123552  | 858  [858] - 2p
% :- not top_hand(hero,  2, _, _, _). % 1098240 | 130 [2860] - 1p
% :- not top_hand(hero,  1, _, _, _). % 1302540 |  43 [1277] - hc
%                                       2598960

% :- not top_hand(hero,4,7,7,_).
% :- not hand(hero,7,spades).
% :- not hand(hero,7,clubs).
% :- not hand(hero,7,diamonds).

% :- not hand(hero,2,clubs).
% :- not hand(hero,3,diamonds).

% #maximize{ HC@3,LC,K : top_hand(hero, _, LC, HC, K) }.
% #maximize{ LC@2,HC,K : top_hand(hero, _, LC, HC, K) }.
% #maximize{ K@1,HC,LC : top_hand(hero, _, LC, HC, K) }.

% Models       : 36
% Calls        : 1
% Time         : 0.265s (Solving: 0.06s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 0.587s

%%-----------------------------------------

% CL-USER> (* (nCr 50 2) (nCr 48 5))
% 2097572400
% CL-USER>

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     shared
% ).

% :- not card(hero,7,hearts).
% :- not card(hero,2,spades).
% :- not card(villan,3,spades).
% :- not card(villan,3,clubs).
% :- not card(shared,10,clubs).
% :- not card(shared,10,hearts).
% :- not card(shared,7,clubs).

%% 990

% :- not top_hand(hero,  9, _, _, _).
% Models       : 0
% Calls        : 1
% Time         : 0.440s (Solving: 0.04s 1st Model: 0.00s Unsat: 0.04s)
% CPU Time     : 0.653s

% :- not top_hand(hero,  8, _, _, _).
% Models       : 2
% Calls        : 1
% Time         : 0.459s (Solving: 0.05s 1st Model: 0.04s Unsat: 0.00s)
% CPU Time     : 0.724s

% :- not top_hand(hero,  7, _, _, _).
% Models       : 171
% Calls        : 1
% Time         : 0.593s (Solving: 0.21s 1st Model: 0.04s Unsat: 0.00s)
% CPU Time     : 1.834s

% :- not top_hand(hero,  6, _, _, _).
% :- not top_hand(hero,  5, _, _, _).
% :- not top_hand(hero,  4, _, _, _).

% :- not top_hand(hero,  3, _, _, _).
% Models       : 817
% Calls        : 1
% Time         : 0.712s (Solving: 0.31s 1st Model: 0.02s Unsat: 0.01s)
% CPU Time     : 2.453s

% :- not top_hand(villan,  8, _, _, _).

%%-----------------------------------------

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     shared
% ).

% :- not card(hero,2,spades).
% :- not card(hero,5,clubs).

% :- not card(villan,2,hearts).
% :- not card(villan,4,hearts).

% :- not card(shared,2,diamonds).
% :- not card(shared,4,spades).
% :- not card(shared,4,diamonds).
% :- not card(shared,5,spades).
% :- not card(shared,5,diamonds).


% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     flop;
%     turn;
%     river
% ).

%% High card.
%% Hero
% :- not card(hero,14,spades).
% :- not card(hero,9,clubs).
% :- not card(villan,14,hearts).
% :- not card(villan,5,spades).
% :- not card(flop,13,hearts).
% :- not card(flop,12,clubs).
% :- not card(flop,8,diamonds).
% :- not card(turn,2,spades).
% :- not card(river,3,spades).

%% Tie
% :- not card(hero,14,clubs).
% :- not card(hero,7,spades).
% :- not card(villan,14,hearts).
% :- not card(villan,5,hearts).
% :- not card(flop,13,diamonds).
% :- not card(flop,12,diamonds).
% :- not card(flop,9,clubs).
% :- not card(turn,8,hearts).
% :- not card(river,3,clubs).

%% Pair
%% Hero
% :- not card(hero,14,hearts).
% :- not card(hero,13,hearts).
% :- not card(villan,14,diamonds).
% :- not card(villan,12,clubs).
% :- not card(flop,14,clubs).
% :- not card(flop,10,diamonds).
% :- not card(flop,7,spades).
% :- not card(turn,5,spades).
% :- not card(river,2,diamonds).

%% Tie
% :- not card(hero,14,hearts).
% :- not card(hero,13,hearts).
% :- not card(villan,14,diamonds).
% :- not card(villan,13,clubs).
% :- not card(flop,14,clubs).
% :- not card(flop,10,diamonds).
% :- not card(flop,7,spades).
% :- not card(turn,5,spades).
% :- not card(river,2,diamonds).

%% Two pair
%% Hero
% :- not card(hero,14,hearts).
% :- not card(hero,13,hearts).
% :- not card(villan,14,diamonds).
% :- not card(villan,12,clubs).
% :- not card(flop,14,clubs).
% :- not card(flop,10,diamonds).
% :- not card(flop,7,spades).
% :- not card(turn,5,spades).
% :- not card(river,5,diamonds).

%% Villan
% :- not card(hero,14,hearts).
% :- not card(hero,12,hearts).
% :- not card(villan,14,diamonds).
% :- not card(villan,13,clubs).
% :- not card(flop,14,clubs).
% :- not card(flop,10,diamonds).
% :- not card(flop,7,spades).
% :- not card(turn,5,spades).
% :- not card(river,5,diamonds).

%% Three of a kind
%% Hero
% :- not card(hero,14,hearts).
% :- not card(hero,13,hearts).
% :- not card(villan,14,diamonds).
% :- not card(villan,12,clubs).
% :- not card(flop,14,clubs).
% :- not card(flop,10,diamonds).
% :- not card(flop,14,spades).
% :- not card(turn,5,spades).
% :- not card(river,7,diamonds).

%% Tie
% :- not card(hero,14,hearts).
% :- not card(hero,13,hearts).
% :- not card(villan,14,diamonds).
% :- not card(villan,12,clubs).
% :- not card(flop,14,clubs).
% :- not card(flop,10,diamonds).
% :- not card(flop,14,spades).
% :- not card(turn,5,spades).
% :- not card(river,5,diamonds).

%% Full house
% :- not card(hero,14,hearts).
% :- not card(hero,13,hearts).
% :- not card(villan,14,diamonds).
% :- not card(villan,12,clubs).
% :- not card(flop,14,clubs).
% :- not card(flop,10,diamonds).
% :- not card(flop,14,spades).
% :- not card(turn,5,spades).
% :- not card(river,13,diamonds).

%%========================================
%% Verifying base combinatorics.
%%========================================

% # = 1326 (ASP 1326)
% player(hero).
% phase(
%     pre_flop_hero
% ).

% Models       : 1326
% Calls        : 1
% Time         : 0.545s (Solving: 0.35s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 2.630s

%%------------------

% player(hero).
% phase(
%     pre_flop_hero;
%     flop
% ).

% Models       : 25989600
% Calls        : 1
% Time         : 1787.134s (Solving: 1786.95s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 14042.855s

%%------------------

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan
% ).

% Models       : 1624350
% Calls        : 1
% Time         : 598.561s (Solving: 597.95s 1st Model: 0.05s Unsat: 0.00s)
% CPU Time     : 4721.497s

%%------------------

% player(hero).
% phase(
%     pre_flop_hero;
%     flop;
%     turn;
%     river
% ).
% :- not card(hero,14,spades).
% :- not card(hero,13,spades).

%% # = 50C3 * 47 * 46
% Models       : 42375200
% Calls        : 1
% Time         : 342.518s (Solving: 342.35s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 2713.299s

%% Without fixing hero hole cards # = 52C2 * 50C3 * 47 * 48
%% Stable model set # = 56,189,515,200

% player(hero).
% phase(
%     pre_flop_hero;
%     shared
% ).

% INTERRUPTED  : 1
% Models       : 483301653+
% Calls        : 1
% Time         : 21047.370s (Solving: 21047.17s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 163333.042s

% :- not card(hero,14,spades).
% :- not card(hero,13,spades).

% Models       : 2118760 % comb(50,5)
% Calls        : 1
% Time         : 70.454s (Solving: 70.28s 1st Model: 0.00s Unsat: 0.00s)
% CPU Time     : 556.327s

%%========================================
%% Cracking the aces.
%%========================================

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     flop;
%     turn;
%     river
% ).

% :- not card(hero,7,hearts).
% :- not card(hero,2,clubs).
% :- not card(villan,14,spades).
% :- not card(villan,14,diamonds).

% Models       : 34246080 % comb(48, 3) * 45 * 44
% Calls        : 1
% Time         : 3354.956s (Solving: 3354.57s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 26542.442s

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     shared
% ).

% :- not card(hero,7,hearts).
% :- not card(hero,2,clubs).
% :- not card(villan,14,spades).
% :- not card(villan,14,diamonds).

% Models       : 1712304 % comb(48, 5)
% Calls        : 1
% Time         : 139.880s (Solving: 139.48s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 1098.447s

%% --project
% Models       : 2041
% Calls        : 1
% Time         : 1233.348s (Solving: 1232.97s 1st Model: 0.02s Unsat: 923.44s)
% CPU Time     : 9664.473s

% :- not defeats(villan, hero).

% Models       : 1493868
% Calls        : 1
% Time         : 300.940s (Solving: 300.55s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 2356.876s

%% --project
% Models       : 1431
% Calls        : 1
% Time         : 107.769s (Solving: 107.39s 1st Model: 0.02s Unsat: 25.49s)
% CPU Time     : 849.403s

% :- not defeats(hero, villan).
% :- not top_hand(hero, 10, _, _, _). %     0 in 0.393s | 0
% :- not top_hand(hero,  9, _, _, _). %   301 in 0.466s | 114
% :- not top_hand(hero,  8, _, _, _). %  1978 in 0.735s | 20
% :- not top_hand(hero,  7, _, _, _). % 27360 in 6.230s | 198
% :- not top_hand(hero,  6, _, _, _). % 32188 in 3.110s | 169
% :- not top_hand(hero,  5, _, _, _). % 40684 in 4.183s | 52
% :- not top_hand(hero,  4, _, _, _). % 44454 in 1.352s | 15
% :- not top_hand(hero,  3, _, _, _). % 64498 in 1.766s | 7

% Models       : 211463
% Calls        : 1
% Time         : 140.936s (Solving: 140.54s 1st Model: 0.07s Unsat: 0.01s)
% CPU Time     : 1102.191s

%% --project
% Models       : 575
% Calls        : 1
% Time         : 50.199s (Solving: 49.82s 1st Model: 0.04s Unsat: 21.74s)
% CPU Time     : 394.944s

% :- not ties(hero, villan).

% Models       : 6973
% Calls        : 1
% Time         : 8.561s (Solving: 8.16s 1st Model: 0.04s Unsat: 0.17s)
% CPU Time     : 64.581s

%% --project
% Models       : 35
% Calls        : 1
% Time         : 1.290s (Solving: 0.92s 1st Model: 0.06s Unsat: 0.00s)
% CPU Time     : 7.566s

%%========================================
%% "Poker Stats" app comparison.
%%========================================

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     flop;
%     turn;
%     river
% ).

% %% Hero: As, Ks
% %% Vilan: ?
% %% Communal: 10c, 10h, 7c

% :- not card(hero,14,spades).
% :- not card(hero,13,spades).
% %% :- not card(villan,3,spades).
% %% :- not card(villan,3,clubs).
% :- not card(flop,10,clubs).
% :- not card(flop,10,hearts).
% :- not card(flop,7,clubs).

%% All possible worlds.
%%------------------
% Models       : 2140380
% Calls        : 1
% Time         : 186.094s (Solving: 185.73s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 1449.885s

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     shared
% ).

% :- not card(hero,14,spades).
% :- not card(hero,13,spades).
% % :- not card(villan,3,spades).
% % :- not card(villan,3,clubs).
% :- not card(shared,10,clubs).
% :- not card(shared,10,hearts).
% :- not card(shared,7,clubs).

% :- not card(shared,14,diamonds).
% :- not card(shared,13,diamonds).

% Models       : 1070190
% Calls        : 1
% Time         : 78.741s (Solving: 78.39s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 616.969s

%% Straight flush.
%%------------------
% :- not top_hand(hero, 9, _, _, _).
% Models       : 0
% Calls        : 1
% Time         : 0.524s (Solving: 0.14s 1st Model: 0.00s Unsat: 0.14s)
% CPU Time     : 1.373s

%% UNSAT

% :- not top_hand(villan, 9, _, _, _).
% Models       : 564
% Calls        : 1
% Time         : 0.565s (Solving: 0.19s 1st Model: 0.03s Unsat: 0.00s)
% CPU Time     : 1.784s

% Models       : 564
% Calls        : 1
% Time         : 0.552s (Solving: 0.21s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 1.593s

%% Four of a kind.
%%------------------
% :- not top_hand(hero, 8, _, _, _).
% Models       : 1980 (0.0009251%)
% Calls        : 1
% Time         : 1.002s (Solving: 0.48s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 4.146s

% Models       : 990
% Calls        : 1
% Time         : 0.537s (Solving: 0.19s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 1.671s

% :- not top_hand(villan, 8, _, _, _).
% Models       : 12516 (0.0058476)
% Calls        : 1
% Time         : 2.521s (Solving: 2.00s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 16.158s

% Models       : 6258
% Calls        : 1
% Time         : 1.018s (Solving: 0.66s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 5.449s

%% Full house.
%%------------------
% :- not top_hand(hero, 7, _, _, _).
% Models       : 53460 (0.024977)
% Calls        : 1
% Time         : 7.045s (Solving: 6.53s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 51.975s

% Models       : 26730
% Calls        : 1
% Time         : 2.340s (Solving: 1.99s 1st Model: 0.05s Unsat: 0.00s)
% CPU Time     : 15.936s

% :- not top_hand(villan, 7, _, _, _).
% Models       : 170508 (0.0797)
% Calls        : 1
% Time         : 23.823s (Solving: 23.30s 1st Model: 0.07s Unsat: 0.00s)
% CPU Time     : 184.739s

% Models       : 85254
% Calls        : 1
% Time         : 10.965s (Solving: 10.61s 1st Model: 0.07s Unsat: 0.00s)
% CPU Time     : 83.696s

%% Flush.
%%------------------
% :- not top_hand(hero, 6, _, _, _).
% UNSAT

% :- not top_hand(villan, 6, _, _, _).
% Models       : 80028 (0.03739)
% Calls        : 1
% Time         : 9.778s (Solving: 9.40s 1st Model: 0.04s Unsat: 0.00s)
% CPU Time     : 74.579s

% Models       : 40014
% Calls        : 1
% Time         : 3.898s (Solving: 3.56s 1st Model: 0.07s Unsat: 0.00s)
% CPU Time     : 28.312s

%% Straight.
%%------------------
% :- not top_hand(hero, 5, _, _, _).
% Models       : 31680 (0.0148)
% Calls        : 1
% Time         : 3.532s (Solving: 3.17s 1st Model: 0.10s Unsat: 0.00s)
% CPU Time     : 25.326s

% Models       : 15840
% Calls        : 1
% Time         : 1.507s (Solving: 1.15s 1st Model: 0.03s Unsat: 0.00s)
% CPU Time     : 9.323s

% :- not top_hand(villan, 5, _, _, _).
% Models       : 64308 (0.03005)
% Calls        : 1
% Time         : 5.192s (Solving: 4.82s 1st Model: 0.06s Unsat: 0.00s)
% CPU Time     : 38.435s

% Models       : 32154
% Calls        : 1
% Time         : 2.524s (Solving: 2.17s 1st Model: 0.07s Unsat: 0.00s)
% CPU Time     : 17.323s

%% Three of a kind.
%%------------------
% :- not top_hand(hero, 4, _, _, _).
% Models       : 142560 (0.06661)
% Calls        : 1
% Time         : 14.794s (Solving: 14.28s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 113.404s

% Models       : 71280
% Calls        : 1
% Time         : 4.545s (Solving: 4.18s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 33.249s

% :- not top_hand(villan, 4, _, _, _).
% Models       : 212760 (0.0994)
% Calls        : 1
% Time         : 8.312s (Solving: 7.80s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 62.147s

% Models       : 106380
% Calls        : 1
% Time         : 2.437s (Solving: 2.09s 1st Model: 0.09s Unsat: 0.00s)
% CPU Time     : 16.709s

%% Two pair.
%%------------------
% :- not top_hand(hero, 3, _, _, _).
% Models       : 801900 (0.3747)
% Calls        : 1
% Time         : 70.361s (Solving: 70.01s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 553.864s

% Models       : 400950
% Calls        : 1
% Time         : 25.751s (Solving: 25.40s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 200.652s

% :- not top_hand(villan, 3, _, _, _).
% Models       : 865392 (0.404217)
% Calls        : 1
% Time         : 40.751s (Solving: 40.40s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 319.687s

% Models       : 432696
% Calls        : 1
% Time         : 19.206s (Solving: 18.84s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 148.832s

%% Pair.
%%------------------
% :- not top_hand(hero, 2, _, _, _).
% Models       : 1108800 (0.5180)
% Calls        : 1
% Time         : 89.510s (Solving: 89.15s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 703.277s

% Models       : 554400
% Calls        : 1
% Time         : 25.254s (Solving: 24.90s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 196.547s

% :- not top_hand(villan, 2, _, _, _).
% Models       : 733716 (0.3428)
% Calls        : 1
% Time         : 17.044s (Solving: 16.69s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 132.122s

% Models       : 366858
% Calls        : 1
% Time         : 7.055s (Solving: 6.71s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 53.142s

%% High card.
%%------------------
% :- not top_hand(hero, 1, _, _, _).
% UNSAT

% :- not top_hand(villan, 1, _, _, _).
% UNSAT

%%   Hero: 2140380
%% Villan: 2140380

%% Wins, losses & ties.
%%------------------

% :- not defeats(hero, villan).
% Models       : 1135172 (0.53036)
% Calls        : 1
% Time         : 51.599s (Solving: 51.23s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 405.283s

% Models       : 567586
% Calls        : 1
% Time         : 22.043s (Solving: 21.67s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 171.082s

% :- not defeats(villan, hero).
% Models       : 839876 (0.3924)
% Calls        : 1
% Time         : 78.041s (Solving: 77.67s 1st Model: 0.09s Unsat: 0.00s)
% CPU Time     : 614.035s

% Models       : 419938
% Calls        : 1
% Time         : 39.581s (Solving: 39.22s 1st Model: 0.13s Unsat: 0.00s)
% CPU Time     : 309.746s

% :- not ties(villan, hero).
% Models       : 165332 (0.077244)
% Calls        : 1
% Time         : 6.986s (Solving: 6.62s 1st Model: 0.13s Unsat: 0.00s)
% CPU Time     : 52.028s

% Models       : 82666
% Calls        : 1
% Time         : 5.439s (Solving: 5.10s 1st Model: 0.05s Unsat: 0.00s)
% CPU Time     : 40.398s

%% Total: 2140380

%% Total: 1070190

%%========================================
%% Hand verification.
%%========================================

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     flop;
%     turn;
%     river
% ).

% %% High card.
% %%------------------
% :- not card(hero,14,spades).
% :- not card(hero,13,spades).
% % :- not card(villan,3,spades).
% % :- not card(villan,3,clubs).
% :- not card(flop,10,clubs).
% :- not card(flop,2,hearts).
% :- not card(flop,7,clubs).

% :- not top_hand(hero, 1, _, _, _).
% :- not top_hand(villan, 1, _, _, _).
% :- not defeats(hero, villan).

% Models       : 419986
% Calls        : 1
% Time         : 9.065s (Solving: 8.69s 1st Model: 0.07s Unsat: 0.00s)
% CPU Time     : 68.943s

% #maximize{ HV@4,P : top_hand(P, HV, _, _, _) }.
% #maximize{ LC@3,P : top_hand(P, _, LC, _, _) }.
% #maximize{ HC@2,P : top_hand(P, _, _, HC, _) }.
% #maximize{ KC@1,P : top_hand(P, _, _, _, KC) }.

% Models       : 4
%   Optimum    : yes
% Optimization : -16 -20 -20 -27
% Calls        : 1
% Time         : 1.481s (Solving: 1.09s 1st Model: 0.01s Unsat: 0.03s)
% CPU Time     : 8.692s

% #minimize{ HV : top_hand(villan, HV, _, _, _) }.
% #maximize{ HV@4 : top_hand(hero, HV, _, _, _) }.
% #maximize{ LC@3 : top_hand(hero, _, LC, _, _) }.
% #maximize{ HC@2 : top_hand(hero, _, _, HC, _) }.
% #maximize{ KC@1 : top_hand(hero, _, _, _, KC) }.

% Models       : 3
%   Optimum    : yes
% Optimization : -8 -10 -10 -14 8
% Calls        : 1
% Time         : 0.787s (Solving: 0.41s 1st Model: 0.01s Unsat: 0.38s)
% CPU Time     : 3.362s

%% Hero hole cards only...

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     flop;
%     turn;
%     river
% ).

%% Hero: As, Ks
%% Vilan: ?
%% Communal: 10c, 10h, 7c

% :- not card(hero,14,spades).
% :- not card(hero,13,spades).

% :- not defeats(hero, villan).

% #minimize{ HV : top_hand(villan, HV, _, _, _) }.
% #maximize{ HV@4 : top_hand(hero, HV, _, _, _) }.
% #maximize{ LC@3 : top_hand(hero, _, LC, _, _) }.
% #maximize{ HC@2 : top_hand(hero, _, _, HC, _) }.
% #maximize{ KC@1 : top_hand(hero, _, _, _, KC) }.

% Models       : 6
%   Optimum    : yes
% Optimization : -8 -14 -14 -13 4
% Calls        : 1
% Time         : 0.594s (Solving: 0.22s 1st Model: 0.05s Unsat: 0.03s)
% CPU Time     : 1.849s

%%========================================
%% Myers' Hold'em book.
%%========================================

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     flop;
%     turn;
%     river
% ).

% :- not card(hero,14,diamonds).
% :- not card(hero,13,diamonds).
% :- not card(villan,8,spades).
% :- not card(villan,7,hearts).

% :- not card(flop,12,clubs).
% :- not card(flop,9,hearts).
% :- not card(flop,4,spades).

%% # = 1980

% :- not defeats(hero, villan). % 1490, 75.25%, 3.04:1
% :- not defeats(villan, hero). % 490, 24.75%
% :- not ties(hero, villan). % 0

% :- not card(flop,12,clubs).
% :- not card(flop,8,hearts).
% :- not card(flop,4,spades).

% :- not defeats(hero, villan). % 470, 23.73%
% :- not defeats(villan, hero). % 1510, 76.26%, 3.21:1
% :- not ties(hero, villan). % 0

% #maximize{ HV@4 : top_hand(hero, HV, _, _, _) }.
% #maximize{ LC@3 : top_hand(hero, _, LC, _, _) }.
% #maximize{ HC@2 : top_hand(hero, _, _, HC, _) }.
% #maximize{ KC@1 : top_hand(hero, _, _, _, KC) }.

% Models       : 6
%   Optimum    : yes
% Optimization : -5 -10 -14 0
% Calls        : 1
% Time         : 0.753s (Solving: 0.40s 1st Model: 0.01s Unsat: 0.28s)
% CPU Time     : 3.093s

% top_hand(hero,5,10,14,na) top_hand(villan,2,8,8,12)

%%========================================
%% ASP Challange

%%=================
%% Assuming Hero has dealer deal just to him the following
%% (Villan is not in grounded model):

% Hero: [14 spades], [13 spades]
% Flop: TBD, TBD, TBD
% Turn: TBD
% River: TBD

%% (1) How many hands are possible for Hero?

% player(hero).
% phase(hero_hand).

% :- not card(hero,14,spades).
% :- not card(hero,13,spades).

%% 2 fixed, comb(50, 5) = 2 118 760

% Models       : 2118760
% Calls        : 1
% Time         : 15.195s (Solving: 15.02s 1st Model: 0.00s Unsat: 0.00s)
% CPU Time     : 118.122s

%%=================
%% Given the following:

% Hero: [14 spades], [13 spades]
% Villan: Hidden, Hidden
% Flop: [10 clubs], [10 hearts], [7 clubs]
% Turn: TBD
% River: TBD

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     flop;
%     turn;
%     river
% ).

% :- not card(hero,14,spades).
% :- not card(hero,13,spades).

% :- not card(flop,10,clubs).
% :- not card(flop,10,hearts).
% :- not card(flop,7,clubs).

% Models       : 2140380
% Calls        : 1
% Time         : 171.329s (Solving: 170.96s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 1351.075s

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     shared
% ).

% :- not card(hero,14,spades).
% :- not card(hero,13,spades).

% :- not card(shared,10,clubs).
% :- not card(shared,10,hearts).
% :- not card(shared,7,clubs).

% Models       : 1070190
% Calls        : 1
% Time         : 86.839s (Solving: 86.46s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 683.874s

%% (2) How many ways can players end up with each of the ten hands?

% :- not top_hand(hero, 10, _, _, _).  %       0         0    0.00%
% :- not top_hand(hero,  9, _, _, _).  %       0         0    0.00%
% :- not top_hand(hero,  8, _, _, _).  %    1980       990    0.09%
% :- not top_hand(hero,  7, _, _, _).  %   53460     26730    2.50%
% :- not top_hand(hero,  6, _, _, _).  %       0         0    0.00%
% :- not top_hand(hero,  5, _, _, _).  %   31680     15840    1.48%
% :- not top_hand(hero,  4, _, _, _).  %  142560     71280    6.66%
% :- not top_hand(hero,  3, _, _, _).  %  801900    400950   37.47%
% :- not top_hand(hero,  2, _, _, _).  % 1108800    554400   51.80%
% :- not top_hand(hero,  1, _, _, _).  %       0         0    0.00%
%                                        2140380   1070190  100.00%

% :- not top_hand(villan, 10, _, _, _). %     24        12    0.00%
% :- not top_hand(villan,  9, _, _, _). %   1128       564    0.05%
% :- not top_hand(villan,  8, _, _, _). %  12516      6258    0.58%
% :- not top_hand(villan,  7, _, _, _). % 170508     85254    7.97%
% :- not top_hand(villan,  6, _, _, _). %  80028     40014    3.74%
% :- not top_hand(villan,  5, _, _, _). %  64308     32154    3.00%
% :- not top_hand(villan,  4, _, _, _). % 212760    106380    9.94%
% :- not top_hand(villan,  3, _, _, _). % 865392    432696   40.43%
% :- not top_hand(villan,  2, _, _, _). % 733716    366858   34.28%
% :- not top_hand(villan,  1, _, _, _). %      0         0    0.00%
%                                        2140380   1070190  100.00%

%% (3) Which player has the most possible wins in their answer sets?

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     flop;
%     turn;
%     river
% ).

% :- not card(hero,14,spades).
% :- not card(hero,13,spades).

% :- not card(flop,10,clubs).
% :- not card(flop,10,hearts).
% :- not card(flop,7,clubs).

% :- not defeats(hero, villan).
% Models       : 1135172 [0.5303600295]
% Calls        : 1
% Time         : 56.975s (Solving: 56.59s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 443.380s

% :- not defeats(villan, hero).
% Models       : 839876 [0.3923957428]
% Calls        : 1
% Time         : 130.460s (Solving: 130.08s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 1021.177s

% :- not ties(villan, hero).
% Models       : 165332 [0.07724422766]
% Calls        : 1
% Time         : 6.076s (Solving: 5.69s 1st Model: 0.18s Unsat: 0.00s)
% CPU Time     : 45.268s

%% 2140380

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     shared
% ).

% :- not card(hero,14,spades).
% :- not card(hero,13,spades).

% :- not card(shared,10,clubs).
% :- not card(shared,10,hearts).
% :- not card(shared,7,clubs).

% :- not defeats(hero, villan).
% Models       : 567586 [0.5303600295]
% Calls        : 1
% Time         : 26.652s (Solving: 26.27s 1st Model: 0.01s Unsat: 0.00s)
% CPU Time     : 208.068s

% :- not defeats(villan, hero).
% Models       : 419938 [0.3923957428]
% Calls        : 1
% Time         : 51.909s (Solving: 51.51s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 407.491s

% :- not ties(villan, hero).
% Models       : 82666 [0.07724422766]
% Calls        : 1
% Time         : 3.799s (Solving: 3.43s 1st Model: 0.08s Unsat: 0.02s)
% CPU Time     : 27.325s

%% 1070190

%%=================
%% Given the following:

% Hero: [7 hearts], [2, clubs]
% Villan: [14 spades], [14, diamonds]
% Flop: TBD, TBD, TBD
% Turn: TBD
% River: TBD

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     flop;
%     turn;
%     river
% ).

% :- not card(hero,7,hearts).
% :- not card(hero,2,clubs).
% :- not card(villan,14,spades).
% :- not card(villan,14,diamonds).

%% tweety
% Models       : 34246080 [comb(48, 3)×comb(45, 1)×comb(44,1)]
% Calls        : 1
% Time         : 2939.992s (Solving: 2939.60s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 23202.949s

% Models       : 34246080
% Calls        : 1
% Time         : 893.693s (Solving: 893.43s 1st Model: 0.03s Unsat: 0.00s)
% CPU Time     : 13950.566s
% Threads      : 16       (Winner: 14)

%% (4) How many ways can the Hero "crack" the Villan's aces?

% :- not defeats(hero, villan).
% Models       : 4229260
% Calls        : 1
% Time         : 1067.548s (Solving: 1067.16s 1st Model: 0.04s Unsat: 0.00s)
% CPU Time     : 8429.888s

% Models       : 4229260
% Calls        : 1
% Time         : 485.299s (Solving: 485.04s 1st Model: 0.03s Unsat: 0.00s)
% CPU Time     : 7570.823s
% Threads      : 16       (Winner: 9)

% player(hero; villan).
% phase(
%     pre_flop_hero;
%     pre_flop_villan;
%     shared
% ).

% :- not card(hero,7,hearts).
% :- not card(hero,2,clubs).
% :- not card(villan,14,spades).
% :- not card(villan,14,diamonds).

% Models       : 1712304
% Calls        : 1
% Time         : 130.037s (Solving: 129.60s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 1027.621s

% :- not defeats(hero, villan).
% Models       : 211463
% Calls        : 1
% Time         : 149.272s (Solving: 148.89s 1st Model: 0.07s Unsat: 0.00s)
% CPU Time     : 1175.416s

% :- not top_hand(hero,  7, _, _, _). % fh

% :- not defeats(villan, hero).
% Models       : 1493868
% Calls        : 1
% Time         : 253.927s (Solving: 253.53s 1st Model: 0.02s Unsat: 0.00s)
% CPU Time     : 2001.251s

% :- not ties(villan, hero).
% Models       : 6973
% Calls        : 1
% Time         : 8.764s (Solving: 8.37s 1st Model: 0.04s Unsat: 0.00s)
% CPU Time     : 66.427s
