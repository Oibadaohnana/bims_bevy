# Bims

A roguelike top-down co-op shooter, written in Rust on Bevy: humans
against the machines. It opens as a window on your desk.

Up to four players share one ship — the **default ship**, the same every
run — and each of them steers one **Bim** aboard it. A crisis of machines
is spreading across the galaxy a hyperlane at a time, and the crew go
where they choose: to a station to trade and to hire, to one the machines
hold to take it back, to a town they are coming for to hold it. Between
places is the **world map**, where the crew choose the next one together;
at every place is a **mission**, which is where the shooting is. **Time
and money are the only resources**: a trip costs days, and the machines
spread a hop every five of them; a machine destroyed pays a bounty, and
the bounty buys guns, armour, hands for hire and the dead back. The run
is over when every player's Bim is dead at once, or the moment the
whole crew, bots too, is down or dead.

**You steer your own Bim** — WASD walk it, the pointer aims it and the
left button fires; Shift sprints (1.8 times the walk, firing
nothing) and Alt dodge-rolls the way it walks, slipping every bolt —
and the crew nobody steers are bots that follow the players into a
fight. [A run](#a-run) is what a run is made of, [the
loop](#the-loop-world-map-travel-missions) is how it goes from one place
to the next, and [the crew](#the-crew) is who is aboard.

The lobby starts [the game](#the-game) straight away — one star system,
the default ship docked at the station you picked, and one clock everything
runs on. The [ship designer](#the-ship-designer) is still there, behind the
`design` command, but a run does not pass through it.

It got here in three steps. Bims was a life sim — Bims on a ship the crew
laid out and flew themselves, cooking, sleeping and keeping the deck clean
on a clock of their own — and the first step (feature 102) switched off
everything of that the shooter does not use, the second (feature 103)
built the loop of travel and missions, and the third (feature 104)
**deleted** what the first had switched off: the needs and the day they
made, the behaviour test room, radiation, every human enemy — the hostile
stations, the raiders, the plunder — and the flown trip, with the helm and
the landing. The git tag `needs-sim-final` is the last commit that has
them.

## A run

What a run is, from the lobby to the end of it.

- **The start.** Start menu, setup or a lobby, the galaxy and a station to
  start at — and then the run: docked at that station on the **default
  ship** (the playtest ship `nix run .#simulation` opens on), with **5 000 a
  player's Bim** in the one pool the crew share and nothing added for going
  alone. The setup tab's money row defaults to it; there is no ship size to
  pick, since there is no ship to lay out. The run opens in its first
  mission, at that station.
- **The crew, and the bots.** Every player steers their own Bim, and
  everybody else aboard is a **bot**: a mercenary hired at a station, or a
  townsperson who joined the crew once their town was held. A bot follows
  the players, takes arms when an enemy is near, dresses a crewmate's
  wounds and answers the two orders every player has — **X** puts an
  attack banner down for it to fight its way to, **Y** calls it back to
  the ship. Each player picks a **class** for their own Bim — engineer,
  soldier, medic, tank or commander, or none — and it levels up in the
  fight; see [Classes and levels](#classes-and-levels).
- **The machines and the Manufacturers.** No station's or town's people
  are the crew's enemies: a place is the crew's home, neutral, held by the
  machines — or held by the **Manufacturers**, the people who built them,
  who are the crew's enemy from the first day; see [The
  Manufacturers](#the-manufacturers). The fights are theirs — Husks, Troopers and Wardens, in waves,
  at tier one, two or three, and Guardians among them at an elite alone — and `nix run .#droids` is how one is looked
  at on its own; see [The fight](#the-fight).
- **The crisis is there from day nought.** The machines' origin is theirs
  the moment the run opens, eight hyperlane hops or more from the crew, and
  every system due by then with it — its jammer standing and its stations
  held — and it spreads a hop every five days from there. An infested
  system's **jammer** shuts the lanes back towards the origin while it
  stands; a system a hop outside the infection is on the **front**, where
  the gear costs more and the machines come stronger. And **every place
  that is neither a trader nor an enemy's is a defence from the first
  day**: arrive and the machines come for it twenty seconds later. See
  [Attack, defend or trade](#attack-defend-or-trade).
- **Money and gear.** A station's desk sells guns and armour at every
  tier, where the place has the trade, and nothing the ship lives on — no
  food, no suits and no medicine, which are everybody's charges anyway —
  and nothing is built onto the ship but a class's mines and sentries.
  The money comes from the Republic's bounty on every machine destroyed,
  and it goes on gear, on hands for hire and on buying the dead back.
  The workbench and the drug lab work as they always did; research is
  gone, and **relics** are what a run collects instead — see
  [Relics](#relics).
- **Nobody eats, sleeps or goes to the heads.** The bunks, the galley,
  the heads, the shower, the hydroponic bay and the cold store are
  **furniture**: drawn as they always were, in the way of a walk as they
  always were, and nothing more — no menu opens on one. And a walk outside
  in a suit doses nobody: there is no radiation.
- **A station's people keep a routine.** With nothing to send them
  anywhere, each is dealt a role when their station's room opens — a
  **guard** walks between the airlocks, or a town's gates and its pad; a
  **trader** stands at the trading desk and steps away from it now and
  then; a **worker** goes between two or three of the benches, research
  desks and shelves; a **civilian** strolls between rooms and stops in
  each — and a round worked out from that station's own fixtures, so every
  station and town the generator makes has one. A role the site has not
  got the fixtures for wanders between spots of open deck instead, or
  open ground on a planet. On an alert they fight or take shelter as they
  always did, and go back to their round when it is over. Every choice is
  off the station's seed and the world's step, so two players' clients
  walk the same rounds.

## The loop: world map, travel, missions

A run is a string of **missions**, one at each place the crew go, with the
**world map** between them: world map, travel, mission, back to the ship,
world map, from the first dock to the end of the run. Nothing is flown:
there is no helm to stand at and no trip to sit through.

- **The world map.** Between missions every player sees it: the system
  map of the stations and settlements round the crew, the galaxy chart
  behind *Galaxy view*, and along the top a list of every place a trip
  can go — every station and settlement in this system, and every one in
  a system a hyperlane joins to this one — each with how long the trip
  is, the day the crew would get there, and what they would find there
  on that day — each row led by what it is, **ATTACK**, **DEFEND** or
  **TRADER**, in red, amber or green — the machines and at what tier, the
  system's jammer, a place already cleared. `M` shows it
  during a mission too, read-only.
- **Choosing together.** Pick a place — on the list, or click it on the
  system map — and **Propose**. Every player still in the game has to
  **Accept**; proposing counts as your own yes, and another proposal
  clears every yes there was. A player who has left the game is not
  waited for, and a player whose Bim is dead still has a vote.
- **Travel costs days, and only travel moves the clock.** A trip is the
  hyperdrive's twenty-minute charge for a jump, and the flight inside
  the system at the ship's own accelerations — from where the crew are,
  or from where the jump lands them, to the place. One hyperlane hop at
  most, and the machines' jammer still bars a jump inward out of a
  system they hold. The moment the last player accepts, the world clock
  goes on by the whole trip in one go and everything that runs on days
  is read at the new day: a system whose day came on the way is the
  machines' when the crew get there, and the waves are sized for the day. The crew arrive
  docked, or landed at a settlement, and a mission begins. **The world
  clock stands still during a mission and on the map**: the day on the
  screen only changes when you travel. **Every trip is at least a
  day**, however close its two ends — the map says *the minimum* beside
  one that would have been shorter — and **the place the crew are at is
  never a destination**: to fight a place again the crew go somewhere
  else first, and a day or more has gone by when they come back.
- **The machines grow with time, and with nothing else you do.** A
  wave is two machines, one more for every **player** — never a bot, a
  mercenary or a townsperson who joined — and one more every **three weeks
  of the world clock**, up to sixteen; a place holds two waves, and one
  more every six weeks. What the crew own, what they carry and how far
  they have levelled make no difference to the machines: getting
  stronger makes the fight easier, and keeping money costs nothing.
- **A mission** begins on arrival anywhere, peaceful or not — a visit to
  a trader is a mission without a fight. Everybody's health is made
  whole, every piece of armour is whole again, every charge is set back
  to what the Bim starts with and every cooldown is ready, and an offer
  of gear still standing is withdrawn (see [the Armory](#the-armory)).
  Gear changes on the map, or aboard the ship on arriving. Everything inside a mission runs on the **mission
  clock**, which starts at nought on arrival: the machines' next wave fifteen
  seconds after the last of one is destroyed, a town's first wave a
  minute after the landing, the class cooldowns.
- **The bounty waits for the place to be cleared.** The Republic pays
  for every machine destroyed — 500, 1 500 or 4 500 by its tier — but
  it is **pending**, "+€ n on clear" along the top, until the place is
  **cleared**: no machine left there and none still to come. Then it is
  paid into the pool, once. Experience is always yours. **A machine one
  of your bots finished off pays only 5%, one a player finished off 110%**
  (`bot_bounty_percent`, `player_bounty_percent` in `rewards.ron`); a
  commander's reinforcements and his Medivac's medic earn as he does, a
  sentry's kill the plain bounty. The experience is the same either way.
- **Back to ship**, at the bottom right. The first press sends every bot
  back to the ship, and every press walks your own Bim there too (click
  the deck to go somewhere else instead). When every player still on their feet has pressed
  it and is aboard, the ship leaves — asking first if anybody would be
  left outside: every player gets *Leave them behind?*, and it takes
  everybody's yes; one no keeps the ship where it is, the presses
  standing, and *Ask again* asks again. A player who is down, dead or
  gone is not waited for. **Left behind is dead**, and a body down
  outside the ship is not carried aboard by leaving — **except after a
  fight won**: once the place is cleared, every crewmate outside who is
  alive and **on its feet** comes home with the ship, and nobody is asked
  about them. One still downed is left behind as before, unless somebody
  revives it or carries it aboard.
- **What leaving does to the place.** A place the crew cleared stays
  cleared. Any other is put back exactly as the mission found it — the
  machines, the dead, the lamps — and its bounty is lost. A town the
  machines were attacking falls to them when the crew leave it before the
  last wave is down, and is an infested place like any other from then
  on.
- **Dying.** A player's Bim that dies is **out**: its gun, armour and pack
  are lost with the body, but its class, level, experience and talents are
  kept. A player's Bim that dies is out for the rest of the mission and
  **comes back when the mission ends**, aboard, **with everything it
  wore** — its gun, its armour, its class and level — and the
  pool pays **5 000** for it, or what it holds if that is less: a Bim
  never waits for money, and the pool never goes below nought. Gear is
  never lost. A bot that dies or is left behind — a hired hand, a
  townsperson who joined — is gone for good but **costs nothing**: only a
  player's Bim is paid for. **Its gun and armour go into the
  armory**. **The run is over when every player's Bim is dead at once** — or at once
  when every player and every bot is down or dead, nobody left to revive
  anybody: a screen says
  so, with the day, and its *Start again* puts the run back to where it
  opened.

## Relics

Research is gone from the game (feature 106): no desk carries a key, and
the workbench upgrades gear from the first day. What a crew collects over
a run instead is **relics** — and since October 2026 a relic is **the
whole crew's**, not one Bim's, and **every relic has a price**. Items are
what a Bim carries; a relic changes how the run plays.

- **Only an elite pays one.** Beating an **elite** — a crowned site, one
  system in ten — offers **three relics** on the reward screen, after the
  departure check and before the map. Nothing else drops one: no trader
  sells them and no site hides a cache.
- **Chosen together, or not at all.** The world map's vote over again:
  any player picks a relic — or **Take none** — and proposes; every player
  still at the keyboard says yes; a new proposal clears every yes. Taking
  none is a fair choice: every relic costs something.
- **Kept, and no cap.** A relic taken is the crew's for the rest of the
  run, and is never offered again. The crew may hold as many as they take.
- **What each does.** A relic works on everybody it names — every Bim,
  the players' own, or the bots (every crew member no player steers:
  bots, hired hands, townsfolk who joined, a commander's reinforcements).
  The crew's relics stand in a row under the top frame at all times, on
  the deck and over the map, as in *Slay the Spire*: rest the pointer on
  one for its name, its boon in green and its price in red, as the reward
  window and the character sheet (**K**) show them too. None slows a revive or cuts the healing,
  so the medic and the Medivac keep their worth. The numbers are
  placeholders, not balanced:

  | relic | what it gives | what it costs |
  | --- | --- | --- |
  | Glass Cannon | +30% weapon damage for everybody | +25% damage taken for everybody |
  | Heavy Plating | -25% damage taken for everybody | -20% move speed for everybody |
  | Hair Trigger | +35% fire rate for everybody | +30% class ability cooldowns |
  | Overclocked Cores | -35% class ability cooldowns | -20% weapon damage for everybody |
  | Bounty Contract | +50% money for every enemy down | -20% damage to enemies |
  | Hunter's Pact | +50% experience for every enemy down | +25% machines in every wave |
  | Drill Sergeant | the bots: +50% weapon damage, -30% damage taken | -20% weapon damage for the players' Bims |
  | Lone Wolves | the players' Bims: +35% weapon damage, +15% move speed | -50% weapon damage for the bots |
  | Black Market | -40% trader prices | +35% HP for every enemy |
  | Adrenaline | +30% move speed for everybody | +15% damage taken for everybody |
  | Salvage Burn | +40% damage to enemies | -40% money for every enemy down |
  | Nanite Mesh | +2 HP a second for everybody on their feet | -15% weapon damage for everybody |

- **Winning.** A run is won by destroying the **Machine Heart** at the
  machines' origin — see [The Machine Heart](#the-machine-heart). For a
  look at the victory screen without the fight, `BIMS_WIN=1` on any command
  wins the run the next time a site is cleared with machines in it. Each
  player's **profile** (`bims/profile.ron` beside the saves, or wherever
  `BIMS_PROFILE_DIR` says) counts the runs won and which classes are open;
  in a lobby the host's decides the classes.

`nix run .#relics` is the droids arena, an elite, with one short wave:
clear it, go back to the ship, and the reward screen offers three relics.
`BIMS_RELICS=glass_cannon,drill_sergeant` gives the crew those relics at
the start, and `BIMS_REWARD=1` opens straight on the reward screen.

## Items

Dota 2's items, beside the relics (October 2026): **four item slots on
every player's Bim**, from the first day — a bot carries none. An item is
bought at a [trader](#the-trader) at the tier the day has reached, moved
in the Armory like a gun (drag it onto one of the four cells under a
player's Bim, or onto another), offered to another player, and kept
through a death like the rest of the loadout. It is bought onto your own
Bim and never into the armory, and **every trader after the one you
bought it at offers it a tier up** — *Upgrade*, highlighted, at the next
tier's price, made in the slot it is in — to tier three. Nothing is
combined. Sold back at a trader it fetches half of everything paid for
it, its upgrades too. The hero panel at the foot of the screen shows
the four two by two at the right of the abilities, each with its key
(`1` to `4`), its tier as pips and an active one's cooldown swept back.

| item | | tier one / two / three | price |
| --- | --- | --- | --- |
| **Blink Drive** | Active (its key): the Bim put where the pointer is, as far as the drive reaches, on ground it can see and walk to. Not within 3 s of a hit | 6 / 8 / 10 tiles, 14 / 12 / 10 s cooldown | 27 000 / 47 250 / 81 000 |
| **Executioner** | Weapon hits may be critical. Rolls on its own beside a soldier's Weak Spot — either may come up, and the bigger multiple counts | 12% ×1.6 / 18% ×1.9 / 25% ×2.25 | 33 750 / 60 750 / 108 000 |
| **Reactor Heart** | More health, and regeneration — faster after 6 s without a hit | +25 / +40 / +60 HP; 0.5 / 1 / 1 HP/s, 1.5 / 3 / 5 HP/s quiet | 27 000 / 54 000 / 94 500 |
| **Override Core** | The class's ultimate plays **one rank higher** than bought (a rank must be bought), up to a **fifth** rank no skill point buys: the Sentry two at once, the Rampage every grenade back, the medic all his healing ×1.5, the Bastion a tile wider and everybody it reached half again as fast, the Reinforcements five in armour — and every number a step on | one tier | 81 000 |
| **Coolant Loop** | Class ability cooldowns shorter; several add, to −50% at most | −10 / 15 / 20% | 20 250 / 40 500 / 67 500 |
| **Pressure Seal** | Health back all the time, hit or not | 0.5 / 1 / 1.5 HP/s | 13 500 / 27 000 / 47 250 |
| **Steady Grip** | The trigger pulled faster | +10 / 15 / 20% fire rate | 20 250 / 40 500 / 67 500 |
| **Long Barrel** | The weapon reaches further | +2 / 3 / 4 tiles | 20 250 / 40 500 / 67 500 |
| **Leech Capacitor** | A share of the damage your weapon does to an enemy (a machine or a Manufacturer) back as health | 8 / 12 / 16% | 27 000 / 54 000 / 94 500 |
| **Arc Coil** | Every 4th weapon hit on an enemy arcs to the enemies nearest it within 4 tiles, machines and Manufacturers alike | 2 / 3 / 4 enemies, 15 / 25 / 40 damage | 33 750 / 60 750 / 108 000 |
| **Field Mender** | Active: every crewmate on their feet within 5 tiles, you included, healed | 30 / 45 / 60 HP, 45 / 40 / 35 s | 27 000 / 54 000 / 94 500 |
| **Reset Capacitor** | Active: every class cooldown ready, every charge full, your other items' cooldowns with them. **Tier three alone**: on no trader's shelf before the run's tier-three day, never upgraded | tier three: 80 s | 810 000 |
| **Ablative Shell** | Active: damage taken ×0.6, and a Warden's lance strips none of your armour | 4 / 5 / 6 s, 60 / 55 / 50 s | 33 750 / 60 750 / 108 000 |

*Coolant Loop*, *Pressure Seal* and *Steady Grip* were relics; they are
in no relic pool any more (a save holding one keeps it).

The prices are placeholders, shared by the players like every price at a
trader. `BIMS_ITEMS=blink:3,executioner:2,heart,core` (any item by its name or
its first word, `field`, `reset`, `ablative`, …) gives the steered
Bim those items at the start of any run, to look at one.

## Attack, defend or trade

**Every place a trip can go is one of three**, and the map says which
before anything else — at the head of its row on the list in its colour,
large on the card with a `?`, and on the system map as the colour of the
ring round its icon and a word under it:

- **ATTACK** (red): somewhere an enemy holds — the machines' stations,
  the Manufacturers' sites, the Machine Heart. Go in and clear it. One
  cleared says *cleared*, its ring faded.
- **DEFEND** (amber): everywhere else — stations and derelicts (a
  town's is an **Area defend**, below), **the place a run starts at among them**, from the very first
  day. **Before day ten the Manufacturers are coming for it**, their
  people and the day's share of their Troopers (see
  [The Manufacturers](#the-manufacturers)); from day ten, the machines.
  The moment you arrive the crew are
  put ashore just inside its airlock and a countdown starts along the
  top — `Prepare: 0:20` — and twenty seconds later the first wave lands,
  at an airlock of the station's (the farthest from yours first, then
  the next, every one but yours in turn) or outside a town's gate, another
  after each is destroyed. You do not fight it alone: the place fields
  **armed defenders** in its own coveralls (two on the first day, one
  more every five, up to eight), and a town's guard and any mercenaries
  there take arms too; everybody else goes indoors. The waves are sized as
  if each defender were another player. **Hold the last wave** and the
  place is cleared, the Republic pays for every machine destroyed, and
  it says *held* on the map — a **town** held stays friendly for good and
  some of its people join your crew; a station or a derelict held is
  simply safe until the crisis takes its system. **Go back to the ship
  before the last wave is down and it falls to the machines**, the bounty
  with it — and whoever is still ashore when the ship leaves is left
  behind, so bring the crew back aboard first. A defender who dies is
  nobody's loss, and a derelict with nobody of its own is never lost
  while you stand.
- **AREA DEFEND** (amber, a flag in a ring of sandbags): a **town's**
  defence. Hold its **FOB** — a green ring where its main street meets
  its first cross street, sandbags round it and a post in the middle —
  for **three minutes** from the first wave. A wave lands **every ten
  seconds** while the time runs, whether or not the last is down — they
  stack — and after the third each comes a second sooner, down to five;
  every wave is the size of the first, whoever of yours has fallen
  since. When the time is up, destroy everything still standing and the
  town is held. The machines fight their way in, cover to cover,
  stopping to shoot whenever they have a shot, the town's defenders hold
  the ring beside you, and **machines standing in the
  ring for twenty seconds with nobody of yours in it take the FOB — the
  run is lost**. Anybody of yours in the ring stops their count; the
  machines driven out puts it back to nothing. Out of sight, the FOB is
  a green glow at the edge of the screen; the top of the screen counts
  the hold down, and a bar fills as the machines take it.
- **TRADER** (green): a trader, visited on the map — see
  [The trader](#the-trader). The machines never come for one.

**There are no mining sites any more**: no mining outposts are built, and
the asteroid belts are scenery, with no mark on the map.

## The Machine Heart

The run is won at the **crisis's origin**, the star the machines began
at. Its system holds one site more than any other: the Heart's
**fortress**, a station the size of the droids arena that is always the
machines' and always at **tier three**. It is reached like any site in the
origin's system — one hop at a time down the lanes, so the jammers on the
way decide how soon — and the galaxy chart marks the origin with a red
diamond once the crew have been in its system or a system next to it.
Pick the fortress on the world map and its card says what it will be **on
arrival**: how many conduits, the core's health, and how many Guardians
the conduits will send — the players decide them, not the clock.

- **The core** stands in the fortress's hub, with two **fabricators**
  beside it; **conduits** stand in the rooms round it, one a room — three,
  and one more a player. **No waves** stand in the fortress or come by
  the clock: every conduit shot down sends **Guardians** in by its
  airlocks, in turn at each — one for the first conduit, two for the
  second, and so on, five for the fifth.
- **Sealed.** While any conduit stands, the core is behind a shell that
  stops every bolt and blow from every side, and it does not fire. A red
  line of light runs from each conduit to it. Bring the conduits down.
- **Exposed.** With the last conduit down, the core sweeps the Guardian's
  beam — harder — at the nearest crew member it can see, player or bot,
  and each fabricator still standing builds a tier-three machine every half
  minute. A fabricator destroyed builds no more.
- **Overload.** Under a third of its health the core sweeps **two** beams
  at once, at two different targets when it can see two, twice as fast,
  and the fabricators build twice as often.
- **Won.** The core destroyed is the run won, whoever of the crew is dead —
  the **victory screen** says how many days the world clock ran, the sites
  cleared, the systems liberated (their jammer cleared), the machines
  destroyed, the deaths and the crew's relics; from there, back to the
  start menu.
- **Leaving** before the core is down puts the whole fortress back as the
  crew met it — conduits, fabricators, core and phase — and pays nothing
  and offers no relic. The dead come back as the ship leaves, and the
  run is lost as ever when every player's Bim is dead at once.

The top bar says it while the crew are there: the core's health, how many
conduits are left.

## The Manufacturers

The machines are months away at the start of a run; the **Manufacturers**
are not. They built the machines, and they defend what they made: some
stations of the galaxy are theirs from the first day — **two at least
within two hyperlane hops of home**, none in the home system, and about
one in ten of the orbital stations anywhere else. Never a town, and never
the crew's home. The world map tags a site of theirs *Manufacturers* with
the tier of what they carry on arrival.

- **Who they are.** People, in **black and gold**, ringed in red and named
  *Manufacturer* on the deck. They walk, take cover, shoot and go down as
  anybody does. Nobody hires one, nobody loots one — no gun, armour or
  money comes off a Manufacturer or a Trooper — and nobody revives one:
  one **down** stays down and dies when its countdown runs out.
- **Before day ten** a site of theirs is a **fixed garrison** — as many as a
  wave of machines would be — and nothing comes after it. Their people
  carry the **laser pistol** and nothing else until day three, a tier-one
  gun from day three, and tier-one armour besides from day six. From day
  five some of the garrison are **Troopers** fighting beside them — a tenth
  of it on days five and six, a quarter on day seven, half on day eight,
  three in five on day nine.
- **From day ten** they have lost the machines: their own people alone,
  in **waves** as a machines' station has them, geared at the tier the
  machines there would come at, and the next wave's ship docks **fifteen
  seconds** after the last of a wave is down.
- **Cleared** the moment every Manufacturer is down or dead, every Trooper
  is destroyed and no wave is left — one down and still counting holds
  nothing up, the clear or the leaving. The Republic pays a Manufacturer's
  bounty like a machine's, for each one down or dead, on the clear; the
  clear offers no relic, since none of theirs is an elite (see
  [Relics](#relics)); and a site left
  uncleared is met afresh next time, at the new day.
- **They attack as well as defend.** Before day ten every place you
  defend is attacked by them rather than the machines: each wave the size
  a wave of machines would be, their people armed by the day as above and
  the same day's share of Troopers beside them — none on day nought, a
  tenth on days five and six, a quarter on day seven, half on day eight,
  three in five on day nine. They come in at the station's airlocks in
  turn or a town's gate, fight you and the place's defenders, and shoot its people; the
  place counts each of them down at its first down, and pays for them as
  for the machines. From day ten the machines come instead.
- **Not the crisis.** A site of theirs is never infested and never a
  jammer, and the machines spreading through its system pass it by.

`nix run .#manufacturers` is the nearest site of theirs with the combat
crew there: `BIMS_MANUFACTURER_DAY=0` for pistols alone, `8` (the
command's own) for the Troopers beside them, ten or more for their waves.

## The Armory

Nothing is kept anywhere. There is no hold for gear, no pack on anybody's
back, no locker to put a gun in and nothing to take off a body — the
armoury, the lockers and the shelves still stand on the deck, and they
are furniture. What the crew own is two things:

- **The ship's holdings**: the money and the **armory** — every weapon
  and piece of armour nobody is wearing. (The research keys counted
  here went in October 2026.)
- **Each Bim's loadout**: a weapon and a piece for the head, the body and
  the legs. That is everything a Bim carries. A class's mines, Healing
  Sentries, satchels and grenades are **charges**, a count the world
  keeps for the Bim (task 127), set back to what its ranks give at every
  mission and coming back on their cooldowns during one.

**Tab** (or the tray's **Armory**) opens the panel on every screen of a
run: a column for each of the crew — its portrait, its class, its weapon
and each piece with its tier and what it has left — and under them the
armory, the money and the keys. **Drag** a thing out of the armory onto a
Bim to put it on (what was there goes into the armory), or off a Bim onto
the armory to take it off; a right-click says the same in rows.

- **Between missions, or aboard on arriving**: on the world map, the
  galaxy chart and the reward screen, anybody may be dressed; during a
  mission only a Bim inside the ship — which is where the crew stand on
  arriving at an attack or a defence — and a Bim out on the deck keeps
  what it has until it comes back aboard. Offers wait for the map.
- **Your own Bim and every bot are yours to dress**; another player's Bim
  is not. To give another player something you are wearing, drag it onto
  their column — or right-click, *Offer to* — and it is **offered**: it
  shows on their column with **Accept** and **Decline**, and moves only
  when they accept, their old one going into the armory. An offer is
  withdrawn when either of you changes that slot, when you take it back,
  or when a mission starts.
- **Armour is never destroyed.** A piece shot down to nothing stays on
  and stops nothing for the rest of the mission, and every piece — worn
  or in the armory — is whole again when the next mission starts.
- **Buying** at a trader puts the thing on a Bim or in the armory at the
  tier bought ([The trader](#the-trader)). Nothing is sold.
- **A player's Bim that dies keeps everything it wore** and comes back
  with it when the mission ends; **a bot's** gear comes home to the armory
  (see [the loop](#the-loop-world-map-travel-missions)).

Whatever the world will not do, it says why in the log: a change during
a mission, another player's Bim, a thing that is not there any more.

## The trader

Gear is bought at a **trader** and nowhere else — no station keeps a desk
for it any more. A trader is a station like any other on the map, marked
*trader* in the list, and on the galaxy chart with a green square round
its star; about one system in ten has one — never two — there is
always one within a lane of home, and **a trader every five hops**: from
any system, a trader's own too, another trader is at most five lanes
away (more are put where the galaxy's roll left a gap). A visit happens **entirely
on the world map**: choose it the way you choose any destination —
everybody accepts — and the trip moves the world clock by its length. On
arrival there is no mission and no room: nothing moves and neither clock
runs while you are there, and the **Trader panel** comes up beside the
map. **Tab** opens the Armory beside it.

- **The shelf**: **every weapon** but the laser pistol **and the
  armour**, always, each **at tier one** until you buy that kind — the
  day lifts no shelf —
  or at its own lowest tier where that is higher (the minigun at two,
  the rail lance at three), priced at the trader's own tier prices. A
  thing bought is gone for that visit, and the next visit puts it up
  again. **Buying lifts that kind**: once you have bought a shotgun your
  shelves sell the shotgun a tier past the best you bought, and nothing
  else moves — up to tier three. A **tank** setting out in armour and a
  **soldier** with an auto rifle are offered those at tier one too:
  only buying lifts a kind. Another player's shelf is its own. Any player buys, with no vote, out of their own
  money — onto their own Bim, onto a bot, or into the armory; what it
  replaces goes into the armory.
- **The items**: every [item](#items) at the day's tier, one of each a visit (SOLD until the next) —
  onto your own Bim's first free item slot, never into the armory. One
  your Bim already carries is offered as its **Upgrade** instead, a tier
  up (to three), at the next tier's price, outlined in that tier's colour.
- **No relic**: a trader sells none since October 2026 — only an elite's
  fight pays one (see [Relics](#relics)).
- **Selling**, the **Sell** tab beside *Buy*: anything you may change —
  your own Bim's weapon, armour and items, a bot's, the armory's — back
  for **half of what it cost**: an item half of everything paid for it,
  a weapon or armour half its shelf price today, into your own money.
  The **laser pistol** every Bim sets out with — yours or a bot's — is
  never sold.
  Nothing is combined any more (October 2026).
- **Closed**: a trader is shut while the machines have its system, and
  opens again once the system is **liberated** — every station, jammer
  and town of it they took, cleared. A trader the crisis will reach by
  the day you would get there says *closed on arrival*. Either way it is
  greyed out on the map with the reason, and cannot be proposed.

Propose anywhere else from the trader's map and, once everybody accepts,
the crew leave and travel.

## Running it

```sh
nix run .
```

That builds the game and opens it. There are nineteen things to run, and each is
a name rather than a flag — `cargo run -- list` (or `bims list`) prints them
all with a line each, and is the build's own answer rather than this table's:

| command | `cargo run` | opens |
| --- | --- | --- |
| `nix run .` or `nix run .#game` | `cargo run -- game` | the whole game, in order: the start menu, setup or a lobby, the world and a station to start at, then [the run](#a-run) — docked where you said on the default ship, 5 000 a Bim in the pool |
| `nix run .#simulation` | `cargo run -- simulation` | straight into the world on a prebuilt playtest ship, docked at a station |
| `nix run .#design` | `cargo run -- design` | straight into the ship design, the playtest ship given, docked where the simulation docks |
| `nix run .#test` | `cargo run -- test` | the simulation somewhere else each time: docked at a random station somebody lives on, in a random galaxy, with a mercenary for hire at the dock |
| `nix run .#test_planet` | `cargo run -- test_planet` | `test` set down on a planet: the same random galaxy, landed at the settlement of a planet whose people are friendly |
| `nix run .#droids` | `cargo run -- droids` | **the fight**: the combat ship — sixteen crew, a gun in every hand, four of them hired field medics — docked at the arena, which the **machines** hold: a wave of Husks, Troopers and Wardens stands about it. They wear nothing, carry nothing and leave nothing to loot; a Husk snaps at arm's length, a Trooper walks into the open with a gun for a forearm, and a Warden's lance **strips the armour off** whatever it hits rather than wounding the body under it. Clear a wave and the next lands at one of the arena's airlocks — the farthest from yours first, then the others in turn — a minute later. Every enemy is a machine (see [a run](#a-run)), so the `combat` command that turned the arena's people against the crew — which would be this exactly — is gone |
| `nix run .#combat_droids_engineer` … `#combat_droids_commander` | `cargo run -- combat_droids_medic` | that **same fight with a class in hand**: the crew member you steer starts as an engineer, a soldier, a medic, a tank or a commander — one command a class, the ship, the arena and the wave `droids`' own, so two of these runs differ by the class and nothing else. It starts at the class's **top level** — the tenth with every one of the class's talents still to choose (seven), or — a ranked kit: the soldier, the engineer, the commander and the medic — the sixteenth with sixteen skill points for its four abilities' ranks; `BIMS_LEVEL=3` opens it at that level instead, `BIMS_RANKS=4,4,4,4` buys a ranked kit's ranks outright, and `BIMS_CLASS` still overrides the command |
| `nix run .#tier2_test` | `cargo run -- tier2_test` | `droids` with everybody's kit at **tier two**: every crew member's gun at it and a full set of armour at it on, and the machines at tier two — nothing at tier one on either side |
| `nix run .#tier3_test` | `cargo run -- tier3_test` | the same at **tier three** |
| `nix run .#droids_planet` | `cargo run -- droids_planet` | the same on a planet: a town held by the machines, the ship set down at its pad, and their lander coming down on the plain beyond a gate |
| `nix run .#crisis` | `cargo run -- crisis` | the simulation **a day before the crisis first spreads**: a random galaxy and a random dock as `test` deals them, and the crisis's origin forced two hyperlane hops from the crew's own star — theirs from day nought, as in every run — with the clock wound to the eve of the day the stars next to it turn. Open the galaxy chart: the origin is red, and since only travel moves the clock, the next ring is red once the crew have travelled a day — the world map says which places the machines will hold on the day you would arrive — and the crew's own system follows five days later; the day any star is due is written under its name when it is picked. `BIMS_CRISIS_DAY=n` moves the day the origin turns, and the rest with it |
| `nix run .#jammer` | `cargo run -- jammer` | the crew **inside an infested system**, two hyperlane hops from where the machines began: every station of it in their hands, a wave aboard the one the ship is tied to, and the system's **jammer** standing — so the chart's route inward is barred in red, a jump that way is refused, and the machines come at tier three because of how near the origin they are. `BIMS_DROID_TIER=1` brings them at tier one instead |
| `nix run .#defense` | `cargo run -- defense` | **a town worth defending**: the ship set down at a friendly settlement with the machines one hyperlane hop away, so the town is next. A minute after the landing a wave sets down outside a gate and walks in; the town's guard and whatever mercenaries live there take arms, everybody else goes indoors, and the red line along the top counts the wave the way it counts a held station's. Hold the last wave and the town is yours to keep. `BIMS_DEFENSE_DELAY=n` is the wait before the first wave and `BIMS_DROID_WAVES=1` a fight short enough to finish |
| `nix run .#guardian` | `cargo run -- guardian` | **the Guardian**: the fight at tier three with every wave one Guardian and two Troopers — the largest machine, a walker behind a shield that stops everything from the front. Get round it |
| `nix run .#relics` | `cargo run -- relics` | **the relics**: the droids arena, an elite, with one short wave of four — clear it, go back to the ship, and the **reward screen** offers three relics to vote on. `BIMS_RELICS=glass_cannon,drill_sergeant` gives the crew those at the start, `BIMS_REWARD=1` opens on the reward screen and `BIMS_WIN=1` (on any command) wins the run on the next clear |
| `nix run .#heart` | `cargo run -- heart` | **the Machine Heart**: the crew docked at its fortress at the machines' origin, everybody in tier-three kit, no waves, a conduit's Guardians — bring the conduits down, then the core. `BIMS_HEART_PHASE=2` opens with every conduit down (none of their Guardians sent), `=3` with the core overloading as well |
| `nix run .#manufacturers` | `cargo run -- manufacturers` | **the Manufacturers**: the combat crew at the nearest site of theirs, on day eight — their people in tier-one kit with Troopers beside them. `BIMS_MANUFACTURER_DAY=0` is pistols alone, ten or more their own waves |
| | `cargo run -- list` | nothing: every one of these printed with a line each, and what the environment adds. `--list`, `--help` and `-h` are it too |

Whichever of them you open, **Esc → Restart → Start again** puts the run back
to the situation it opened in — the fight as it was dealt, the run as the
lobby started it — without leaving the window; the end
screen, when the crew are down, offers the same as *Start again*. Nothing on
disk is touched by it, and a saved game is still there to load.

The `cargo run` forms build from the working tree, which is what you want
while editing — `cargo run --release -- test` is the same thing on the
release profile, which is the build to play on; `nix run` builds from the
git tree. The window wants a
display and a GPU: winit and wgpu open the window system's libraries and the
Vulkan loader at run time, and `shell.nix` puts them on `LD_LIBRARY_PATH`,
so a bare `cargo run` outside the shell may open nothing — `nix develop` or
`nix-shell` first, or let `./check` do it.

The rest:

```sh
nix develop            # a shell with the toolchain and the runtime libraries
nix build              # the binary lands in result/bin/bims
nix flake check        # builds it, runs every crate's tests, checks formatting
./check                # the quick tier, under a minute: the fast crates' tests, the pinned
                       #   numbers, clippy, and four windows opened and screenshotted
./check full           # the same deepened — every crate's tests, every sweep, every
                       #   window, the probes, the flake
./hidden <command>     # runs it on a headless compositor — nothing opens on the desktop
```

`bims --self-check` prints whether this build agrees with the constants the
fixtures pin — the design hash, the world checksum, the money arithmetic —
and exits non-zero if it does not.

## The builder

`nix run .` opens here: what comes *before* the ship and the world — a start
menu, a game setup screen, and a lobby. It is `crates/app/src/screens/builder.rs`,
a screen of its own, and the game's screen knows nothing about menus. Only the
World tab touches the galaxy, through `crates/lobby`: the galaxy is generated
and drawn there, and the rest of the screen runs without it.

**Play** goes straight to game setup. **Create lobby** opens a room other
people will one day be able to walk into, and a code field beside the two of
them is the other half of that: type a code, press Join, and it refuses out
loud. That refusal is the honest state of things — there is no transport yet.

The lobby is the settings screen with company. Down the left, four slots: you
in the first one as host, the rest open and waiting. Across the top, the room
code, set like something you read out to somebody. On the right, the same
tabbed tool the setup screen uses:

- **Game setup** — the money each Bim brings at €2 500, €5 000 or
  €10 000, €5 000 unless you say otherwise. Nobody starts with stores:
  everybody's money goes into **one pool**, and [the run](#a-run) opens on
  the default ship with it. There is no ship size to pick any more: a run
  is always the default ship.
- **World** — which galaxy, and where in it the game starts. A **seed** —
  any whole number up to a `u64`, typed in decimal, with **New seed** to
  draw one — and a **galaxy type**: two-arm spiral, spiral, elliptical or
  round. Under them the galaxy itself, six hundred stars on a canvas —
  every one of them a system with at least a station and a planet to
  land on: drag to
  pan, scroll to zoom, hover to read a star's name and class, click one to
  open its system on the right — the star at the middle, its planets and
  belts on their orbits, its stations as squares, each named with its kind
  and the body it hangs off. A system with a station usually has more —
  up to six, two of a kind allowed. No station's people are enemies — every
  human is friendly in a run — so a crew may start at any of them. Stars
  with no station are dimmed, since the game cannot start there; they can
  still be looked at. **Start here** on a station makes it the pending
  start, marked on the map and named in the tab's header; **Random start**
  picks one anywhere. A new seed or a new type is a new galaxy and forgets
  the start.

  No distances, no travel times, nothing about what a station is like: the
  lobby is where a start is chosen, not where a system is explored.

One settings object sits behind both copies of the tool, so what you pick on
the setup screen is what the lobby shows and the other way about.

**Start** needs a station picked — until there is one the button is disabled
and says so — and then opens [the run](#a-run) on every machine in the
lobby. What crosses is numbers and nothing else: the money each Bim brings,
how many players there are, which slot you are, the seed in its two halves,
the galaxy type, and the star and the station the game starts at. What a
game is *started with* is what crosses into the simulation, and no
strings do — which goes for the euro sign as well, so what is written down is
a bare count of euros.

### The multiplayer seam

`Net` in `crates/app/src/screens/builder.rs` is the whole of it: `create`,
`join`, `push`, `leave`, and what the screens read back from it. It is a local stand-in with the
shape a transport will have, and **nothing in the screens reaches past it** —
the slot list is drawn from what `net` says the players are, not from what the
lobby knows about itself. Giving it a socket is meant to be a change to that
object and to nothing else.

Two things it already does properly, because they are easy to get wrong later:
the settings tool knows how to be read-only, for a guest in somebody else's
lobby — a guest sees the host's world, can pan, zoom and inspect it, and can
**Suggest** a station, which rings that star on everybody's map, but cannot
change a thing — and the settings themselves are plain numbers: a sum of
money, a tile count, a seed, a type and two ids. Nothing but numbers can
cross into the simulation anyway.

**Your Bim has a name, and so does everybody's pointer** (feature 60). The
Game setup tab has a **Your Bim** field, host's and guest's alike: what
you type there is what your crew member is called — over its head, in
its panel, in the log and the diary — and blank keeps the crew's own
name for the berth (James, Kate, Priya, Tomas). The lobby's crew list
shows each player's Bim's name beside them, in the colour that player's
pointer and route will be drawn in; the names are dealt with the slots
at Start, one said late still lands, and a save keeps them. Over the
ship, every other player's **pointer** is drawn where it is —
a see-through arrow in their colour with their Bim's name beside it, over
the tile they are over on *your* screen whatever they have zoomed or
turned, gone when theirs is off the ship or on the map. It is sent twenty
times a second at most and never through the host: a pointer is a picture,
not an order, and it is in nothing that has to agree.

**No two Bims look quite alike, and yours wears the hair you gave it**
(feature 62). Every body is one of five **builds**, from slight to
sturdy — six per cent of the body scale a step, enough to tell two
apart standing together — and wears one of eight **hairstyles** in one
of six **colours**: cropped, long, bald, bob, bun, mohawk, ponytail or
curly, in dark, brown, black, blond, red or grey, drawn from above on the
crown and again on the deck when the body is down. The first two of
any crew are the pair they always were (a dark crop with the pale yoke,
brown hair down past the collar with the mauve one); everybody after —
a crew of five, a station's people, a mercenary — is dealt a
look off its index, the same on every machine and off no dice, so a seed
that pins a probe is not moved by it. The Game setup tab's **Hair** row,
under the name field, is the chooser: the figure as it will stand on
the deck, a button a style and a swatch a colour, everybody's to pick
like the name; it goes out with the name, is dealt with the slots at
Start and one picked late still lands. A look is a picture and nothing
else — the reach, the hit test and the checksum are the same whatever
the hair — and a save carries it on the body.

**The host's world is the world** (feature 67). With company every copy
of the world is the host's, checked by checksum every couple of seconds;
when a guest's copy parts from it — a bug, a build not quite the host's
— the guest says so in the log (*Your world has drifted…*, *Catching up
with the host…*), asks the host for its world, and the host sends the
whole game as the text a save is, which the guest then plays on from as
its own crew member (*Back on the host's world.*). The same is how a
load works with company: **only the host can load** — a guest's Load is
greyed with the reason — and a game saved for a different number of
players than are in the room is refused (*That game was saved for N
players; M are here.*); a load that goes replaces the world on every
machine, and guests still in the yard are brought into the game with it.
A world in flight is a few megabytes, so it is a hitch on both ends, the
way a load is. A game of one loads as it always did.

**A shaky connection plays smoothly** (task 148). A guest's world moves
on the host's word, a step at a time, and over Wi-Fi or a busy line that
word comes in clumps — nothing for a tenth of a second, then six frames'
worth at once — which used to freeze the guest's world and make it jump.
A guest now holds the host's steps a moment in a **network buffer** and
plays them out at the world's own pace, one a frame: it starts at two
frames, grows by however long it last ran dry, and gives back what it
did not need after ten quiet seconds, so a steady line costs next to
nothing. The price is latency — the guest sees the world, and its own
orders come back, that much later — and the Esc sheet's **Network
buffer** slider is how much a player will pay at most (0.4 s to start
with, up to a second, 0 for off, as before). It is the guest's alone; a
host and a game of one never wait. Nothing is skipped or reordered, so
the checksums agree as they did. Over a line shaken by up to 150 ms,
a guest that played no step on 603 of its frames and three at once on
164 played exactly one on 1076 of 1087 with the buffer on, 95 ms behind.

## The ship designer

> **Not in a run any more** (feature 102, [A run](#a-run)): the lobby's
> Start opens the run on the default ship, and the yard is the `design`
> command's alone. What follows is the yard as it still is.

The lobby's **Start** used to go here: the whole crew laying out **one ship**
together, on a tile grid, before anybody is aboard. It is not a command of its
own any more — a design phase with no lobby in front of it has no station to
start at, and the page says so rather than picking one: opened without a star
and a station in its query, or with ones the galaxy has not got, it shows
**Nowhere to start** and a link back to the lobby. It never falls back to
another spawn.
It is `crates/app/src/screens/designer.rs` over `crates/ship`.

**No Bims exist during this phase.** Placing a part and taking it off again
are both instant and free: nothing has been welded yet, and the money is only
being promised. That stops the moment everybody accepts — after that, every
change is a Bim's work.

A tile is 52 world units and holds at most one part per **layer**, of which
there are four:

- **Structure** is the frame the ship is built on. It needs nothing under it,
  and it is the first thing anybody lays: nothing else goes down without it.
- **Floor** is the deck plating you walk on. It needs structure.
- **Object** is the one thing standing in the tile — a wall, a bunk, an
  engine. Most need deck; the hull parts stand straight on the frame, which
  is what lets a ship be skinned before it is floored.
- **Utility** runs *through* a tile without filling it: power conduit, which a
  body walks over and a hob can stand on.

A wall is on the object layer like everything else, because a wall and a bunk
in one tile is equally nonsense. What each part needs under it is one column
of the part table, and removal reads the same column backwards: nothing comes
out from under something that is standing on it.

### Laying one out

The designer **opens on a ship**: the playtest ship, laid out in the middle
of whatever build area the lobby chose, whole and flyable, as a gift — the
pool is what the crew brought and none of it has been spent. Everything on
it can be moved, taken off or added to like anything you placed yourself,
and taking a given part off puts its price in hand as any removal does.
`?preset=0` opens an empty grid instead, which is what the harnesses use;
a grid too small for the ship (under twenty tiles) opens empty too.

The palette down the left is grouped the way a ship is thought about — hull,
systems, crew, galley, heads, storage, bay — rather than the way the enum is
numbered. The rows are built from what `shipdesign` says exists, so a part added
and forgotten in the grouping turns up under **Anything else** instead of
quietly not existing.

- **Click** to place. **Drag a rectangle** for the things you fill an area
  with — deck plating, conduit — **drag a line** for both kinds of wall,
  and **drag a diagonal** for the two corner pieces: a **diagonal wall**
  and a **diagonal outside wall** fill half their tile, cut at forty-five
  degrees, and a drag lays a staircase of them one tile a step, every
  piece turned the ghost's way. That is how a hull gets a pointed bow or a
  chamfered corner, the way spacecraft hulls in games of this kind do. To
  the rules a corner piece is a whole tile — one object, a body cannot
  pass, and the hull one seals its tile against radiation exactly as a
  straight plate does, since the exposure fill is four-neighbour and a
  staircase touching corner to corner is tight; only the picture is a
  triangle, and which half is solid is the piece's turn: **R** walks it
  round the four corners of the tile.
  **Deck plating lays its own frame**: there is no separate structure tool,
  because to anybody but the connectivity check the frame and the deck are
  one thing. A plated tile has both; the hull stands on the frame, deck or
  no deck.
- **Right-click peels the top part off a tile** and only that — a hob comes
  off and the deck stays, a second click takes the deck, a third the frame.
  A right-dragged rectangle peels every tile in it by one. Across the tiles
  the parts come off top down, or a tile's deck would be refused because the
  next tile's hob, in the same drag, was still standing on it.
- **R** turns the ghost a quarter clockwise. The footprint and the use spots
  turn with it, and the palette shows the turned size.
- **Middle-drag** or the pointer against the window's edge (WASD too until
  October 2026)
  pans; the **wheel** zooms. The view is clamped
  to the build area and a margin, and starts showing all of it.

A drag is applied as a run of single edits, and **a failing one is skipped and
counted rather than fatal**: a rectangle of deck over a half-floored room is
meant to fill the gaps and pass over the rest.

The ghost is green where the tool would go down and red where it would not,
and the readout by the pointer says the same thing in the same colour before
the click rather than after it. Resting on a placed part rings it and shows
its **use spots** — the tiles a Bim will stand in to use it. Those are only
ever shown for the part under the pointer; drawn permanently they would fill
the deck with markers.

### What it costs, and what it checks

Across the top is what is left of the crew's money, beside what there was to
start with. There is **one pool**: every Bim's purse goes into it, a lone
player gets a fixed bonus on top because a ship for one costs what a ship for
four does, and every part anybody places comes out of the same figure. Each
part has a price in euros. A removal hands back the **whole** price, and what
is left over at the end is money rather than cargo — it is not aboard, and it
does not count towards what the ship weighs.

What is left is always **worked out from the design**, never decremented as
parts go down: a refused edit, a removal and a replayed run of edits cannot
drift apart from what is actually on the ship. The sum itself —
`money_per_bim × players`, plus the solo bonus — is `crates/economy`, so the
game and the native server that will one day be authoritative arrive at the
pool the same way, in whole euros, with overflow an error rather than a wrap.

### Buying what the ship will live on

Under **Station** on the right is what there is to buy: vegetables, tofu,
medkits, bandages, a suit, and whatever gear the spawn station trades in
— at its own two prices a
unit — what one *costs* here and what the desk *pays* for one, the
station's ask and bid (*Trading*, under the game, has the sum). Buttons
move one, ten or a hundred, and putting a thing back hands back what it
cost — nothing has left the dock, so there is nothing to lose on the
deal; the bid is shown, and is what a sale will fetch once the game has
started, but nothing is sold in the yard, only put back. The yard buys at
**tier one**: the tier chooser on the gear rows is the docked trade
window's, since the design phase's purchase takes no tier.

Two things bound a purchase, and the station's shelf is neither of them.
Supply is unlimited; what refuses an order is **the
pool** — goods come out of the same money the hull does, so a player who
spends everything on plating has nothing to load it with — or **the ship**.
Goods are stowed: food in a cold store, and everything else in a locker —
the armoury, the drug lab, the suit locker, and the **shelf**, which is
locker class too since the money rework took the shelf class away with
the materials that filled it. The readout under the rows is how full each
class is, and a ship with no cold store cannot take food at all however
much money there is.
Each of those is not a count but a **grid**, ten cells across and as many
rows as the parts aboard add up to — a shelf or a cold store is ten by
ten — and every thing kept there covers its footprint of it: a pistol a
row of two, a rifle seven, a sniper rifle the whole width; kevlar four
by four, a helm two by four; a crate of vegetables one by two, a block
of tofu four by four. Goods that stack take one footprint a stack — ten
vegetables to a crate, five dressings to a box — so what a shelf holds is
its cells times the stacks. A container's window lays its grid out as it
is: drag
a thing to move it, press `R` on the way to turn it, and a thing goes in
only where there is a run of cells for it — so a full hold is tidied,
not counted.

What is bought is aboard from the moment it is bought. It is in the design
hash, so a purchase clears everybody's Accept the way a wall does; it is in
the ship's mass, so the acceleration on the handoff screen already accounts
for it; and a shelf with something on it cannot be taken off until it is sold.

### Everything costs money, and weighs what the table says

Every part has a **price** in euros and a **mass** in the same table
(`shipdesign::parts`), and the two are deliberately unrelated: a wall
costs what a wall costs and weighs what a wall weighs. There was a third
column — a **recipe**, so many units of metal and components — and the
mass was that recipe added up; the money rework took the materials away
and wrote each part's mass down as exactly the number its recipe used to
come to, so nothing about how a ship flies moved.

That is what makes construction a **purchase** rather than a move. The
price of a part leaves the crew's pool and the part's mass arrives on the
ship; take the part off again and **the whole price comes back** — there
is no wastage, no scrap and no scrapping penalty, and the crew are
neither richer nor poorer for building and unbuilding. A ship's mass
changes by trading at a station, by building and deconstructing, and by
crew coming aboard or leaving — nothing is burnt in flight.

**Money works anywhere a part is concerned.** Goods are bought at a desk,
so they want a dock; a part does not, because euros are not a shelf: a
site is paid for docked, holding station or landed. The design phase
happens docked at the spawn station, which is why a part can go down
instantly there; out in the world a site is walked to and worked at, and
the price leaves the pool when the work begins (*Building, aboard*).

The rule is written down and tested, against every part in the table, in
`crates/shipdesign/src/materials.rs`: `site_price` says what a site
costs — a plating site is the floor and, where the tile has no frame, the
structure under it — and `refund_for` says what comes back.

### What it checks

Down the right is what is wrong with it. An **error** blocks Accept; a
**warning** is the design saying what it will be like to live with. Resting on
a row rings the tiles it names — a highlight, not a tooltip: nothing is said,
and it goes the moment the pointer moves.

The errors are:

- the ship is in more than one piece;
- fewer bunks or chairs than there are players;
- no table, cold store, worktop, hob, dishwasher, toilet or basin;
- somewhere a Bim has to stand is off the ship, has no deck, or is blocked;
- parts nobody could walk between — over deck, through doors, which count as a
  way through;
- an engine firing into the ship: the tiles straight behind its bell have to
  be open space, so an engine stands in the skin with its bell over the
  edge. The designer washes every engine's exhaust onto the deck as you
  build — flame-coloured out into space, warning-red with a cross on every
  tile of the ship it would cook — for the ghost as well as for what is
  placed, so you see it before the checks panel says it.

The warnings are no engine, no engine on some axis, no hydroponic bay, no
broom locker, no helm, nothing to eat aboard, a consumer nothing powers, a
conduit run drawing more than its reactor makes — and **radiation**.

**Having no engine is never an error**: a ship that cannot fly is still a
ship you can live on, and refusing to let a player accept one would be the
design phase having an opinion about how to play. An engine is worked on
from whichever side a body can get at — it has hull on three sides more
often than not — so it needs one free tile round it, not a particular one.

### Radiation, which is a warning and is louder than the errors

Hull keeps it out. The **outside wall**, the **airlock**, the **sensor array**
and the **engine** block shield; a plain internal wall does not, and neither
does a door — so a ship skinned in ordinary walls is a ship whose crew are
being cooked.

It is worked out by flooding in from outside the build area, four ways only,
through everything that does not shield. Every tile the flood reaches and
finds a part in is **exposed**, and the deck is tinted over every one of them
— not when you rest on the row, but always, because a player who has not
looked at the checks panel is exactly the one about to accept a ship with a
hole in it. The row sits first in the list and is styled louder than any error.

It does not block Accept. That is deliberate: it is a decision about how to
play, and the design phase does not take those. It only makes sure nobody
takes it by accident. Two hull parts meeting at a corner seal that corner —
the flood is four-way, so a hull drawn as a staircase does not leak at every
step of it.

The warning is the designer's own, and the designer's alone now: nothing in
a run doses anybody, in a suit or out of one, since the radiation went with
the rest of the old game (feature 104). The designer is untouched by that,
and still says what it always said.

That required list was a **mirror of what the room's chains walked to** — a
meal was a cold store, a worktop, a hob, a table with a chair and a
dishwasher; a night a bunk; a trip to the heads a toilet and then a basin.
It was never a design. The chains went with the needs (feature 104) and the
list stayed, since the designer is untouched and a run never passes through
it; `crates/shipdesign/src/validate.rs` still says at the top what it was a
mirror of.

### Power

The reactor makes it, the battery holds it, and everything that draws — life
support, the helm, the sensor array, the cold store, the bay, every
door, every bench, and **every lamp** — has to be **wired**: a tile of it
carries conduit, and that conduit runs to a reactor. Conduit is on its own
layer and runs *through* a tile, under whatever is standing in it, so there
is no adjacency rule to learn: drag a run of it under the things that need
it, as you would drag deck, and end the run under the reactor. A conduit
run is a **network**; two runs that both end under the same reactor are
one.

Two warnings come of it, and neither blocks Accept, for the same reason the
flight warnings do not — a ship that cannot run its cold store is still a
ship you can live on, for a while. **Nothing powers this** rings every
consumer with no live conduit under it — a lamp among them, and a lamp
with nothing powering it is a lamp that gives no light. **This run draws
more than its reactor makes** rings the run: a reactor is two and a half
thousand a minute, an engine burning flat out takes a thousand of that,
and the lamps are the biggest of the day-long draws — a wall light 25, a
standing light 40, so the playtest ship's six wall lights and standing
light are 190 of the 327 it draws all day, more than its benches
together. What the reactor has over after that is what the engines get, so
a ship lit from end to end on one reactor pushes a little less hard; and a
ship whose day-long draw goes over what its reactors make — a reactor
taken off for a rebuild, a run cut, or a big hull hung with lamps and
benches on one basic reactor — runs on its batteries until they are
flat, and then it is *the brownout*, below.

### Accepting

Each player has an Accept, disabled while anything is an error. An Accept is
recorded **against a hash** of the design — the parts sorted by position and
kind, with the ids left out, so the same layout gives the same number whatever
order it was built in. Any successful edit by anybody changes that hash and
clears every Accept, so there is no way to be holding one for a ship that is
no longer on screen.

When everybody's Accept matches the current hash the phase ends: editing locks,
and **the game starts**. Solo, one Accept settles it.

### The two crates behind it

- **`crates/shipdesign`** is the rules and nothing else: the part table, one
  `apply` that is the only way a design ever changes, `validate`, and
  `design_hash`. It renders nothing and knows nothing about a window — the
  native server that will one day be authoritative has to agree with the
  game about what a legal ship is, and `design_hash` has to come out
  **identical on both**. That is why nothing in its data or its hash is a
  `usize` or a float. Its unit tests are plain `cargo test`; the same
  constants are checked all at once by `bims --self-check`.
- **`crates/ship`** is the screen's half: the camera, the pointer, the ghost,
  the draw buffer, and `Session`, which is the design phase and the game it
  turns into. It decides nothing about what may be placed — it asks.
- **`crates/economy`** is the money: whole euros, the shared pool, what a
  station charges for a unit of anything, and which class of hold it goes in.
  Every sum in it is checked — overflow is an error, never a wrap.

Its multiplayer seam is the same idea as the builder's. `Net` in
`crates/app/src/screens/designer.rs` has a transport's shape, every Edit and
every Accept goes through it carrying the design hash it was made against,
and the host end applies messages in arrival order and reports a refusal back
to whoever sent it. No click handler touches the editor directly.

## The game

Nothing comes before it any more. The lobby's Start opens the run
**docked at the station the lobby picked**, on the default ship, with the
pool in the crew's hands, and from then on there is one world, one clock
and one loop.

That is worth saying plainly because it is the decision everything else
hangs off. The star system, the ship in it, and everything aboard it all
advance together in `World::step`, which moves the world by a sixtieth of
a minute and nothing else — a minute of the **mission clock**, since the
world's own clock stands still until the crew travel (see [the
loop](#the-loop-world-map-travel-missions)). The crew are in it, and they
are **the room**: the Bims' simulation laid out on your ship, one Bim per
player from the first step and whatever bots the crew have taken on,
walking, working the doors, seeing, shooting, bleeding and dressing each
other's wounds inside that step — and the people of whatever station the
ship is tied to are a room of their own beside it. Construction is in the
same step, where it is on ([the crew build what you lay
out](#building-aboard)); a second clock would be two simulations that
disagree, and the failure would read as a ship in two places.

The room's fixtures are drawn with the room's own pictures — the fridge,
the hob and its pot, the pan, the bunk with its rails — turned with the
ship, and the Bims are named over their heads by the app (`CREW_NAMES` in
`crates/app/src/names.rs`, unless a player named theirs). The galley, the
bunks, the heads and the bay are drawn **as a fresh room draws them** and
never change from it: nothing aboard is cooked on, slept in or grown in
any more, and they are furniture a walk goes round.

Two limits, honestly stated: every fixture is used from the south — a
bench with a wall below it is a bench nobody can reach — and the room's
navigation cannot walk a one-tile corridor, so a ship built with them is
a ship whose crew freeze in them. The default ship has neither problem,
which is not an accident.

### Light, and the dark

A deck no light reaches is **dark**, and in the dark the crew see ten
tiles. Two lights are built like any part: a **wall light** hangs from a
bulkhead or the hull (the designer says so if it has no wall at its back)
and reaches seven tiles, a **standing light** stands anywhere and reaches
nine; both **draw power** — 25 and 40 a minute — and want conduit under
them like the cold store does. A lamp on no live network is dark, and every
lamp on the ship goes out in a brownout (*The reactor, the batteries and
the brownout*), which is how a brownout is noticed: the deck goes dark
round the crew and they see their ten tiles. The lamp is whole — it comes
straight back with the power — where one shot out is glass. A station's
lamps are its own and stay lit whatever the ship is doing. A wall stops light the way it
stops the eye, so a room behind a bulkhead is dark for all the lamps on
the other side, and the shadow a lamp casts round a table is a shade —
light, but there. What the crew see is drawn as it is: a smooth cone from
each of them with the walls' straight edges, the dark shaded where they
see it, the fog where they do not. The default ship and every station
come lit; a ship of your own wants lamps, or its corridors are ten tiles
long to whoever walks them.

### Comforts

Three parts do nothing but make a deck nicer to look at — a **small
plant**, a **big plant** and a **picture**, under *Comforts* in the
designer and *Furniture* on the Build tab. They used to lift the crew's
**Surroundings**, the need that followed the state of the deck round a
Bim; with the needs gone they are pictures and nothing else. The default
ship carries a picture beside the bridge door and a small plant by the
bunk, and every station sets a few of its own about — a big plant in the
middle of the hub, a small one in the mess and the rec room, a picture in
the quarters and the rec room. The picture hangs from a wall the way a
wall light does.

### Locking doors, and what the enemy do about it

Every airlock is a door now: right-click one for the same four words a
bulkhead door has — open, close, lock, unlock — and a locked airlock is a
wall, to a walk and to the eye. A machine with nobody it can get to goes
for the doors: it walks to the nearest locked door it can reach and
**smashes** it, fifteen seconds for a bulkhead door and thirty for an
airlock, one body to a door, with a bar over the door saying how long the
lock has left and a heave heard every couple of seconds; then the lock
gives and the door opens for it. The crew never smash a door and never
seal themselves in — they are yours to send. Docked, the station's doors
are the same doors on both sides of the passage: a lock you set stops the
station's people, and a lock they set you can lift from the panel.

### The galaxy behind the map

On the map, **Galaxy view** swaps the system for the galaxy the lobby
showed: the star the crew are at is ringed in green, the lanes out of it
are lit, and the stars they reach are ringed — those are where the next
trip can go. **A trip follows the hyperlanes, one hop at most.** Pick any
star, near or far, and the chart draws the **shortest route** to it along
the lanes, but the world map only ever offers the places a hop away, so
getting across a galaxy is a chain of trips with a system to stop in at
every step of it, rather than one from anywhere to anywhere. **Every
system has somewhere to stop**: a station at the least and a planet with
a town on it. Clicking a star a lane away picks its first place on the
list — a trader first — and the card offers the trip, so a system is
flown to straight off the chart; the panel in the corner says how many
lanes off a star is when it is further. That panel says the star's
**tier** too, and the chart shows it at a glance: a star whose machines
come at tier 2 is ringed in amber, one at tier 3 in red, tier 1 is not
marked, and zoomed in (and round the ship's own star) every star has
`T1`, `T2`, `T3` — or `T1–2`, where the tier is rolled a site — written
under it. A **green square** is a system with a trader, faded while it is
closed.

A jump puts the ship well inside the new system, and nothing of the old
one comes along — its stations, its people, its construction sites — and
everything of the ship's does. **The system remembers, though**: come back
and it is as the crew left it — a station they cleared still cleared, the
key they took still off its desk, the chart still charted, the dead still
dead — where a system never visited is as the galaxy rolled it. (A place
left *uncleared* is put back as its mission found it; see [the
loop](#the-loop-world-map-travel-missions).) **System view** puts the map
back.

The **hyperdrive** is still a part like an engine — a two-by-two block on
deck, wired like anything that draws, and **bolted to a main engine** or
the designer says so — and still researched in the designer's tree,
though no key opens anything any more. A run's jump does not ask for one: every trip across a lane
is quoted with the drive's twenty-minute charge, whatever is aboard.

### A town on a planet

A **rocky planet** or an **ice world** has ground, and a trip to its
**settlement** ends with the ship set down on a **landing pad**; a gas
giant and a belt have none. There is no space any more: the planet is the
whole of the surroundings. Beside the pad stands a **town** of five to
thirty people, on one of three **biomes** — **desert**, **temperate** or
**arctic** (an ice world is always arctic; a rocky planet is one of the
other two) — and no two towns are laid out the same. It is built like a
**fort**: a **wall** runs round the whole of it, the pad is set into the
west wall — the ship docks into the wall as it docks into a station's
hull — and two or three **gates**, each a street's width with a pier of
wall either side, open where a street meets the north, the east or the
south wall. The streets themselves are the town's own, drawn from its
seed: a main street east from the pad, two or three cross streets, a
side street north and south of it or not, the hall somewhere on the main
street, and the rest poured along the frontages. Its **houses** stand
along the streets, a few bunks
each with a door onto the street; the **gathering hall** is its mess, a
galley along the north wall and tables with a chair for everyone; a
**bathhouse** holds a toilet, a basin and a shower for every twelve
people; the **trading house** is where trade is done across the desk as
at any station — the ship's airlock opens onto the ground through the
town's gate, and the Station button is the same — and the **watch house**
by the pad has two sandbags before it and the town's **guard** on its
round. A desert or temperate town has **fields** in a belt beyond the
houses, strips of soil, and an arctic town **greenhouses** full of
hydroponic bays — pictures now, like the bays aboard. Standing lights
line the streets, and by day the whole ground is lit. Over the ground the
town leaves inside its wall is the **wild**, and it is what stops a Bim
rather than a line on the ground: copses, a lake or a river and a few
boulders in temperate country; cliffs of rock, cactus scrub and one oasis
in a desert; outcrops, a frozen lake and firs in the arctic. A Bim off
the ship can walk any way it likes until a tree, a cliff, the water or a
wall is in the way — there is always a way from the pad to every door,
every field and every gate, and never a pocket it cannot get to.

The town is not the whole of the ground. It stands on a **plain** ten
thousand tiles across, and the ship is set down on open ground with the
plain on every side of it: walk round the hull, or out through either
gate, and keep going. What is out there is the planet's —
cliffs, water, forest too dense to push through, the odd tree and rock —
and it is what confines a crew member, not any edge; the plain's own
edge is a rim of cliff further off than anybody will walk. The ground is
made as it is walked and seen, never all at once, and the view reaches
**sixty tiles**: the camera cannot be pulled out further than that on a
planet, the ground is drawn that far from the middle of the window and
no further, and a crew member sees that far over open country. The whole
plain is always drawn, and what the crew do not see of it — looked at
before or not — is under the same fog as the deck, with the same smooth
edges: the shadow a cliff or a forest throws is the cliff's edge, not a
stair of tiles. Nothing out there is black; the fog hides who is standing
on the plain, not the plain. A walk out into it is a walk like any other — right-
click the ground, however far, and the crew member goes leg by leg. A
town the machines hold is ringed in red on the map and fought through
like a held station, their lander coming down on the plain beyond a
gate. Leaving a planet is *Back to ship*, as it is anywhere.

### The reactor, the batteries and the brownout

Every step, what the wired reactors made less what the wired consumers drew
goes into the batteries, and the **Power** line on the ship panel says so:
so much drawn of so much made, and what the batteries have of what they
hold. A battery on the run arrives empty and fills at the surplus; one
taken off takes what was in it. A ship drawing more than it makes drains
its batteries at the difference, and when they are flat it **browns out**:
everything optional stops, and the essentials, life support and the doors,
run on off the reactor's own output. The world opens with the batteries
full, since the ship has been sitting at a dock. The log says *Brownout*
the step it starts and *Power restored* the step it ends, and the ship
panel's Power line carries *— brownout* in between.

What stops, all of it recoverable and none of it lethal:

- **The lamps go out.** Every lamp on the ship, at once, and the deck is
  dark round the crew: they see ten tiles, and so does anybody looking for
  them. That is the alarm — nothing is drawn for a brownout but the dark.
  The lamps are whole, and light again the moment the power is back.
- **The benches and the research desk stop**: no order is placed and the
  AI thinks about nothing until the power is back.

Nothing about the air: life support is essential and runs on, and nobody
dies of a brownout. A dark ship and a stopped bench are something to work
back from, not a game over.

### Making things

> **Since task 113 nothing is made and nothing is kept**: the workbench's
> upgrades and repairs, the drug lab's medkit and every hold went, and
> what the crew own is [the Armory](#the-armory). What follows is how it
> used to work.

The crew make **one thing**, at one bench, and one mechanism does all of
it. A **recipe** is a bench, what goes in, what comes out and how long it
takes; the **drug lab** turns two **vegetables** into one **medkit** in a
quarter of an hour, and that is the whole table. Everything else a ship
carries is **bought** — see [Trading](#trading) — because since the money
rework there are no materials: no ore, no metal, no components, no
galvum, no emitters. A part of the ship costs euros and a gun costs
euros, and what a crew do to get richer is travel, fight and trade rather
than run a production line.

The **workbench** still stands, and it is still worked at: two weapons or
two pieces of armour of a kind at one tier go onto it and one of the next
tier comes off, a day later (see *Armour*, and *Two of a kind go onto the
workbench* in `crates/world/CLAUDE.md`). The **armoury** still stands too,
as the cabinet the crew fetch a gun out of and stow one into. Neither
makes anything.

What turns the one recipe into an errand is a **target**: on the
Management tab's crafts table, the medkit's row carries a *keep so many*
number, stepped up and
down, and while the hold has fewer medkits than the number — and two
vegetables to make one with, and room for it, and a powered drug lab
aboard — the work list offers *Making things*. A Bim walks to the bench,
stands at it for a quarter of an hour, and the vegetables come out of the
hold and the medkit goes in. The log says what was made. If the vegetables
were sold while the Bim stood there, nothing is made and the log says that
instead. The target is a command like a deal, so every player's ship is
making the same thing.

Medicine is known from the first day. **Crafting conserves mass**: a
medkit weighs exactly two vegetables, and nothing vents anything — the
smelter that used to is gone. The drug lab draws power, and it stops in a
brownout.

### Research

**Research left the game** with the relics (feature 106): the ship's AI
researches nothing, no station's desk carries a key, and nothing in a run
waits on the tree — the workbench upgrades gear from the first day. The
yard's palette is what it was, the tree's starting knowledge deciding it
as before. The research desk stays a piece
of furniture on every station. See [Relics](#relics).

### Mining, on foot

**There is no mining.** There was: a belt held at laid a field of
asteroids out on the ship's own tile grid, rocks were marked with a pick
pointer, and a Bim in a pressure suit walked out and dug ore and galvum
out of their cores. The money rework took the whole of it away with the
materials it fed — there is no ore, no metal, no galvum and nothing to
smelt them at — so the **Actions** tab, the marks, the *Mining outside*
job and the site itself are all gone.

What is left of it: the **asteroid belts** are still bodies in a system,
still on the map, as scenery — there is simply nothing to go to one for,
and the map no longer marks one. The **mining outposts** are gone too:
none is built any more. And the **pressure suit** and the **suit
locker** are still aboard, because a walk outside is still how a
construction site beyond the hull is reached (see *Building, aboard*):
the Bim takes the suit from the locker, walks to the deck inside the
airlock, goes out onto a navigation grid of its own — a hundred tiles
every way about it, rebuilt as it moves — works at the site, and comes
back in and hangs the suit up.

One body outside at a time: the airlock is one Bim's while a walk is on.
A right-click on the deck does nothing to a Bim outside: the walk is what
brings it in. What used to bound a walk outside was **radiation** — the
suit let a quarter of the open dose through — and it went with the rest
of the old game (feature 104): nobody is dosed out there any more.

### Building, aboard

> **Off in a run** (feature 102, [A run](#a-run)): nothing is built onto the ship but a class's mines and
> sentries, and the Build tab is not shown.

The ship goes on being built after the design phase — by the crew, paid
for out of the crew's money, and nothing is instant. The **Build** tab at
the bottom left is the palette: the parts by category — *Structure* for
the deck, the walls and the hull and the ways through it, *Furniture*,
*Production* for the benches, the armoury and the bay, *Galley*,
*Hygiene*, *Power*, *Ship systems*, *Propulsion* — each row with what the
part **costs**, dimmed to a warning where the money will not stretch, and
a search box over the lot for when you know the word and not the
heading. Pick a part and it is in your hand: a **blueprint** of it follows
the pointer over the deck, the part's own picture shown through, green
where it would go and red where it would not, with the reason at the top
left — something standing there, no deck under it, or the one the designer
would have given: it would go, but a Bim could then not get at the hob.
`R` turns it, a click lays it out, a right-click or Escape puts it down.
The rules are the designer's, asked of the ship as it will be once
everything already laid out is built, so a wall on deck that is itself
still a blueprint goes — the crew take the sites in the order they were
laid out, and the deck is there by the time they come to the wall.

A site laid out is a **construction site**, in blueprint blue, and the
crew work it as one job on the work list, *Building*. There is no
hauling: since the money rework a part is **paid for** rather than made,
so nothing is carried anywhere and no shelf is emptied. A Bim stands
beside the site and puts it together — a few minutes for a wall and a
couple of hours for a heavy engine, the length worked out from the price
— with a **charge bar on the tile** while it works, and the part is on
the ship: the room the crew live in is laid out again under them with the
new wall a solid in it, the new shelf somewhere to fetch from, and
nobody's errand is lost for it.

**A part's price leaves the pool when its site is begun**, not when it is
laid out, and the crew are never allowed to begin more than they can pay
for: a site whose price the money left over — less every site already
begun — will not cover simply waits, and the log says *not enough money*.
**Taking a part off gives the whole price back.** And a site is paid for
**anywhere**: docked, holding station or landed on a planet, because
euros are not a shelf and a crew with money in hand can build with it
wherever they are. The Build tab's **Laid out** list says what each site
costs and what is under way, with a way to call each one off; a site
called off costs nothing.

A site can be **outside the hull**. Plating laid out against the skin, an
outside wall on it, a thruster in the void beside the ship: any site with
no tile beside it that a body can stand on from the deck is reached from
outside, and the crew take the suit from the locker, go out through the
airlock, walk round the hull on the outside's grid to the tile beside the
site, build there, and come back in, through the same one-at-a-time
airlock. No tools are needed for any of it, only the money.

**Nothing was ever built on a ship that was moving**, and a ship never
moves now: it is docked, landed or holding station wherever the crew are,
so a site can be laid out and worked at any of them.

### Seeing where you are

The system you start in is **charted**: every planet, belt and station the
lobby's chart showed is on the map from the first step, drawn as what it is —
a rocky world with its continents, a gas giant with its bands and ring, an
ice world under glare, a belt of rocks; an orbital wheel, a refinery's tanks
and stack, a mining rig in its rubble, a broken derelict, a relay's dish — on
faint rings that show their orbits. **You are the reticle**: the ship is
the map's origin, drawn as a little hull pointing where it points, inside
a breathing cyan ring with a tick at each compass point that sits over
every icon — a docked ship is on top of its station's, and the ring is
wider than the station's — with *You · Docked · the station*, *You ·
Alongside the belt* or *You · Open space* written over it in your own
colour, the way the galaxy chart tags your star. **Where you have already
been is ticked**: a small grey tick at the shoulder of every station,
planet and belt the ship has actually stopped at — docked, landed or
holding beside — and on the galaxy chart a grey ring round every star you
have been to, so a map you have travelled about says where you have been.
Click a station or a settlement and the world map's list picks it, with
the trip quoted; *Propose* puts it to the crew (see [the
loop](#the-loop-world-map-travel-missions)). A planet you are alongside
is drawn under the hull in the ship view, as the ground.

A **station is a place**, not an icon: every one in the system is a hull on
a grid of its own — thirty-four to seventy-two tiles across, laid out
from the blueprint's seed with the same parts and the same rules as a
ship. The one you start at is a **hub and four arms**: a square hub of
open deck in the middle, a corridor five tiles wide running out of each
side of it to a docking lobby with an airlock in its far wall — the west
one is the port you dock at, and its lobby is the reactor room too, with
the trading desk by the door — and the rooms hung off the north and south
arms two deep a side: the mess and the crew's quarters, the heads and the
laboratory with its bays, the rec room and the research room, the storage
and the cargo shelves, each behind a bulkhead with a two-tile doorway, the
outer rooms opening through the inner. A barricade of **sandbags** stands
across three of each corridor's five tiles a few tiles out from the hub —
cover to crouch behind, and low enough to walk and shoot over. A Bim
**in cover** — close behind sandbags with an enemy beyond them, or
leaning out round a wall to shoot — dodges half the bolts that reach it,
and wears a **curved blue wall** on the deck on the side its cover is
against and a small blue shield at its shoulder, so who is in cover, and
from which way, reads across a fight. **Every
other station is generated from its seed**, so the same dock is the same
building every visit and no two docks are one building: two or three long
corridors with two to four short ones across them, rooms between them
and along their outer sides, some of the space between left open to
space as a courtyard, a corridor now and then running on to a docking
arm, and most of them with **wings** growing off that core — a boom run
out past an arm, sometimes turning a corner, rooms along it and a module
at its end — so the silhouette is a core with things sticking out of it.
The airlocks are anywhere on the hull, in a room's outer wall as often as
at the end of a corridor, and the machines come in by every one but
yours in turn, so a wave can land in a store, in somebody's quarters or
at the far end of a boom. Which way it lies, how wide every corridor is, how deep every room
and where every door and airlock goes are all the seed's. There is
always a way round: a loop of corridor round a block of rooms, so a
crew can come at a room from two sides. A relay is the smallest, an
orbital the biggest; every one has the reactor room with the trading
desk by the port, the same rooms and a research desk. The six drawn
plans it used to be are still there, for the rare seed the generator
cannot lay out: the hub; the **pod**, the smallest, a squat bar with one corridor two wide
and a single resident; the **cross**, two fat bands meeting in a hall,
narrow corridors up the arms, three living there; the **spine**, long and
thin with a corridor three wide the length of it and four aboard; the
**ring**, a square ring of corridor round a void with the rooms outside it
and six aboard; and the **comb**, three arms off a spine to a docking bay
each, five aboard. They differ in size, in how narrow the corridors are
— a barricade fits only in a corridor three wide or more — and in how
many people live there; they all have the same rooms, the reactor room
with the trading desk by the port, and a research desk. The seed decides
how many bays, shelves, tables and batteries; a bigger station gets more
of each. Bulkheads and doors, the helm, the shelves and the shower
are drawn as themselves rather than as coloured blocks, in the room's
palette, so a station reads as a building and a ship as a ship — and so
are the reactor, the battery and life support.

**A door in a bulkhead is a powered door**, two tiles along the bulkhead
and one deep — the whole of a doorway the crew can walk — and turned with
`R` to stand in a bulkhead running either way. It opens by itself for
whoever walks up to it and shuts a moment after the doorway is clear, so the crew
and the residents come and go through them without being told, and the
navigation plans straight through an unlocked one. Four things are on a
right-click: **hold open**, **close** (let it look after itself again),
**lock** and **unlock** — each an errand that walks the Bim you steer to
the panel. A locked door is a
wall until it is unlocked: no route is planned through it, the readout
says `Door · locked`, and the leaves wait for anyone standing in the
opening before they shut. A hydroponic bay, where a station has one, is a
**run of six trays**, one a tile — a bay standing north–south is six
trays down. In the ship view a station is drawn where it is and as big as
it is, so it *approaches*: from far off a plate with its icon on it,
nearer its hull tile by tile, and within fifty tiles of its hull the
people who live there — two on most stations, one on a relay, nobody on a
derelict, whose room opens all the same so its fixtures are drawn — are
the room's Bims again, up and about between the room's own pictures of
its fixtures, with names over their heads. Leave and come back and they
are there again — **the living ones**: whoever died there stays dead, and
a mercenary hired away is not there to hire twice. **And the dead are
still lying there**: every body stays where it fell, in the coverall it
wore and with whatever it had on it, for as long as the station stands —
so a town the machines came for is a town with bodies in its streets when
the crew come back to it. The ship **docks beside it, airlock to
airlock**: a trip ends at the berth, the ship turned so its airlock
faces the station's, the two collars — each stands half a tile out of
its skin — meet as a tube between the hulls, and both doors are drawn
parted. The game opens docked at the first station somebody lives on, never
at a derelict. Escape opens a settings sheet with every key on it. An airlock has to be in the skin for that — one on the deck is a door
to nowhere, and the checks say so. And docked, **the two are one deck**: the
ship's and the station's on one navigation grid with the passage between
them, so a right-click on the station's deck sends the Bim you steer
through the airlocks. The station's people stay on the station: they live
in a room of their own, each on the round its role deals it ([A
run](#a-run)). The Management tab is the crew's own and reaches nobody
ashore. Who is who is on their backs: the crew wear the ship's blue
coverall and the station's people the station's orange one. Leaving takes
the deck apart again, once the crew are back aboard — see *Back to ship*
under [the loop](#the-loop-world-map-travel-missions).

Beyond the chart, the ship sees `VISION_RANGE` with the crew's own eyes,
which out here is almost nothing, and a great deal further with a **sensor
array**; what it sees is discovered, shared by the whole crew and never
forgotten.

Most systems have a station now — about three in five, and often more than
one — so a start is rarely far from somewhere to go.

### Speed, and who decides

**1×, or paused, and nothing else** (task 119): a minute of the game's
clock is a real second. The **Esc** sheet's **Pause** button pauses and
its **Resume** sets the world going again, and **`** sets it going too;
**Space** paused until it became the engineer's remote trigger (task
154) and later the ultimate. The top frame says **Paused** while the world is. **A pause by any player is a pause for everybody**, and the world runs
again only once every pause is lifted: the player who needs it stopped is
the player something is going wrong for. Travel on the world map still
puts its whole length on the clock in one go.

### The HUD

The screen over the deck is kept to what a player needs to act on in the
next few seconds (feature 107); everything else is a key or a button away.

- **Top left, the crew's portraits**: one cell each, players and bots, eight
  to a row — the initial (and the number a bot is named by), a thin health
  bar with the armour's blue on the end of the green, and the level under
  it, `–` for no class. Your own is outlined in the green; a tick in the
  corner is a player who has pressed *Back to ship*, a red cross one who is
  down, and a greyed cell saying **out** is dead, or a player's Bim waiting
  to be bought back. Click a cell to pick that Bim as a click on it on the
  deck would; click your own twice for the character sheet.
- **Top centre, one frame**: the day, the pool, the bounty waiting on the
  place being cleared (`+€ n on clear`, while there is any), and **the one
  warning that matters most** — the machines' wave or the countdown to the
  next, then the alarm, then a blade at your Bim, then being recruited,
  then your standing order to the crew — with `+2` for however many more
  are up; rest on it for all of them. **Paused** beside it while the world is.
- **Bottom centre, the hero panel** — your own Bim: the level in a circle
  ringed by how far through it you are, the health bar — tall, with the
  number beside it — the experience
  (`Lv 4 · 50 / 250 XP`, `Max` at the tenth, *No class* without one), the
  class's keys as boxes, and a **+1** while a talent is waiting to be
  picked (it opens the character sheet). **Critically hit** — downed, or
  under twenty — its frame beats red and *CRITICAL* stands beside the
  number (feature 110). Downed, the panel greys over and says so, with
  the seconds left to be revived in.
- **Bottom left, the tray**: **Armory** (Tab) — the [Armory](#the-armory)
  panel, every crew member's gear and the ship's armory, money and keys
  — **Squad**, every bot with its class, level and
  health and the orders the crew take (Attack, Retreat, Follow me, and a
  commander's Fall back and Stand ground), and **Map**, the world map as
  **M** is; and **Trade** while the ship is at a desk. The panel of the one
  pressed opens upwards; pressing it again folds it away.
- **The character sheet** — **K** — on the left: the class and level with
  the experience, the class picker while it may still be changed, each
  part's health, what is worn with its tier and what is left of it, the
  weapon and its tier, and the talent tree, where a level's pick is spent.
- **Bottom right**, *Back to ship* — `Returning · 1 / 2` once pressed, the
  players aboard who have pressed it of the players the ship waits for —
  and what just happened directly over it, four lines at most, each gone
  eight seconds after it came; experience comes in a line a source a second.
- **Right**, the panel of a crewmate you have picked: its health, its
  perils and its sheet. Your own is the hero panel's and the character
  sheet's, so it has none.
- Rest the pointer on the deck and a small readout beside it names what
  is under it and the tile.

A player whose Bim is **out** watches the mission through a crewmate's —
the portraits pick whose — under a *You're out* banner with the buyback's
price and the pool, and has no hero panel and a tray of the Map alone.
What the ship view draws over the ship — the plain deck or the
electricity — is on the Esc sheet's menu. The Work, Management, Build,
Research and Ship tabs are gone from the HUD, and the crew keep to what
they were left on.

### Trading

> **Since the trader (task 114) nothing is bought or sold across a
> station's desk**: gear is bought at a trader, on the map ([The
> trader](#the-trader)), and nothing is sold. What follows is how the
> desks worked; the prices it describes are the ones a trader's shelf is
> priced at.

> **In a run** (feature 102, [A run](#a-run)) a desk sells the crew
> **gear** — guns and armour, every tier — and nothing else: the food,
> the suits and the medicine on its shelf are greyed out. It still buys
> whatever it buys.

**Money only works while docked**, because a station is where there is somebody
to buy from. Holding station beside one is not docked — that wants an airlock —
and out between them the pool buys nothing at all. Supply is unlimited; what
bounds a purchase is the money and the hold. Since the money rework trading
is most of the economy: there are no materials and almost nothing is made,
so what a crew carry, wear and shoot with was bought at somebody's desk.

**Every station charges its own prices, and two numbers a thing.** There
are two ideas of what a unit is worth, and they are kept apart on
purpose. The **book value** (`economy::trade_price`) is what a thing *is
worth* — the same everywhere, and used only to value what the crew own:
what they set out with, what they are worth now, how many hands a
station has for hire. Nothing is ever bought or sold at it, and the
machines never look at it. What a station's
desk actually charges is its **market price** (`economy::market`): a
**quote** of two numbers, the **ask** — what one costs bought here, the
*Costs* column — and the **bid** — what the desk pays for one, the
*Pays* column. The bid is always under the ask, so a thing bought and
sold straight back at one desk always loses money, and the whole of the
trading game so far is that two desks lean different ways.

The sum, in whole euros and rounding down at every division:

    mid  = book * (100 + kind_bias + local_bias) / 100
    half = max(1, mid * SPREAD_BP / 20 000)
    ask  = mid + half
    bid  = max(1, mid - half)

`kind_bias` is what the *kind* of station does to the price, per cent —
a hand-written table, starting values to be tuned: a mining outpost pays
well for food and medicine and is dear on gear; a refinery is the
cheapest place to buy a gun; an orbital sells food cheap; a relay is dear
on everything and pays well for food and bandages; a planet's settlement
sells food cheap; a derelict has no market at all — nothing to buy, and
nobody to sell to.
`local_bias` is the station's own lean, one small whole number per cent
a resource in `−15..=15`, rolled by the generator off the station's seed
for every resource whether it stocks the thing or not, and in the galaxy
checksum beside its shelf — so two outposts in one system are two
different outposts, and two players on one seed see the same numbers.
`SPREAD_BP` is the desk's cut, a thousand basis points: five per cent
either side of the mid, and never less than a euro. A desk **inside the
front** — within three hyperlane hops of the machines — charges over the
odds for what a fight is fought with on top of all that; see *The
crisis*.

**The station you start at leans only the way its kind does**: the
generator rolls it a local bias like any other, and the world sets it to
nothing (`World::start`, and the yard's Station panel with it), so an
opening pool buys the same at a kind of station whatever the seed
rolled, and what the crew set out with is worth its book value.

**Not every place sells gear.** Beside its shelf of goods, every place
with a market rolls **two trades** off its own seed, each independently
and each two in five: a **weapon trade** — handguns, shotguns, auto
rifles, sniper rifles and schwords — and an **armour trade** — helms,
kevlar and leg guards. A place may have both, one or neither, a derelict
has neither, and the trade is all of its list or none of it: nowhere
sells three of the five guns. Which trades a place has is said before you
go there — on the map — and on a line of its own at the top of the trade
window, because a
crew looking for a rifle are choosing a station rather than a system.

**Every tier is on sale where its trade is.** A weapon or a piece of
armour comes at tier one, two or three, and a market that deals in it
deals in all three: the price is the book times **one, four and sixteen**
(`economy::TIER_PRICE`), on the ask and the bid alike. The trade window's
gear rows carry a **1 2 3** chooser for it — pick a tier and the row's
*Costs* and *Pays* are that tier's, and a line already in the cart goes,
since a tier changed under a line would be that line at another price.
Selling needs no chooser: a sale gives up the **lowest** tiers in the
hold first and the desk pays for each thing at its own tier, so a crew
that has combined two pistols into one keeps the good one until they
choose to let it go.

**Tier one's book prices are hand-written**, and the armour's are a rule:
a handgun 1 500, a shotgun 3 000, an auto rifle 4 000, a sniper rifle
5 000, a schword 5 000; and **a hundred euros a point of health** for
armour — leg guards 1 000, a helm 1 500, kevlar 2 000. The labour rule
that used to price everything a bench made went with the production
chains it was written to keep honest: with one recipe left there is no
chain to print money along, and every book value is a number somebody
chose.

The shelf is a window — **Trade** on the tray, docked, opens it in the
middle of the screen and the cross, Escape or leaving shuts it — and
what is *on* it is two rules deep. The kind's is the ceiling: nothing at
a derelict — there is nobody aboard to sell it. Under that each station
keeps a shelf of its own, rolled off its seed: **vegetables, tofu and
medkits are on every one**, because a station where the crew can buy
nothing to eat and nothing to treat a wound with is a trap, and each of
the rest is there or not, so two refineries stock different things and
there is a reason to go to the other one. A row the
station does not sell is greyed with its buy buttons off, and stays,
because what is aboard can still be sold there, at the bid. Every station
somebody lives on buys anything; a derelict buys nothing, since there is
nobody at its desk. A station is not yet *for* anything beyond the way
its kind leans and the trades it rolled — a theme would replace the roll,
not the ceiling.

### Mercenaries

At a station somebody lives on there may be a **mercenary**
living among them: a body in an olive coverall rather than the station's
orange, with a **?** floating over its name, which is how you know it can
be spoken to. How many depends on what your ship and its hold are worth
against what you set out with — at the start it is one at some stations
and none at the rest, and a crew that has got richer finds more, up to
four. Right-click one and the row is **Hire — see the terms**: it walks
your Bim over and opens the **Hire** window, which says what they carry
— the gun, and every piece of armour worn — and **what a month of them
costs**. The fee is the kit: a pistol and nothing else is about two
thousand a month, a sniper rifle in full armour about twenty thousand,
and every mercenary asks a little more or a little less than the kit is
worth, up to fifteen percent either way. **Hire** wants your Bim within
two tiles and the first month's fee in hand — the window greys the
button and says which is missing; a bunk is furniture, and there need not
be one free. Hired, they walk out of the station's room and onto your
deck, a crew member from then on: they work and fight like the rest, and
take orders like a crewmate (only the Bim you steer takes yours). **They are paid by the month**,
every thirty days out of the crew's money, and the log says so. A month
the money will not cover is a month owed: the log says they are unpaid,
and at the next berth they walk off — back into the station's room, for
hire again when you can afford them. The `test` command always has one
for hire at its dock.

**About a third of them are field medics** (feature 86), and the Hire
window says so at the top before it says what they carry. A field medic
is hired for its trade and not for its gun: **four thousand a month** on
top of the kit, and for it you get somebody whose whole business in a
fight is your crew. It has **none of a medic's own skills** — no beam,
no drone, no circle, no class at all — and what it does instead is this:

* it keeps to the **far end of its weapon's reach** and shoots from
  there, so it is still standing when somebody needs fetching;
* the moment a crew member goes down within about eighteen tiles, it
  goes and **picks them up**, walks them clear of the fight at six tenths
  pace with its fire held, sets them down somewhere nothing can see them,
  and **revives them there**;
* it revives in a medic's **four seconds**, where anybody else takes ten
  (see *Getting hurt*).

Before anybody is down it is an ordinary crew member with a gun, and it
works and fights like the rest.

### The trading desk

> **Gone with the desks' trading (task 114)**: the desk still stands inside
> every station's port, as furniture, and does nothing.

Every station keeps a **trading desk** just inside its port — a wooden
counter with a ledger and a terminal on it, against the corridor's north
wall. The station is traded with across it: **Trade** on the tray still
opens the shelf, but the rows are live only while the Bim you steer
stands within two tiles of the desk, and until then the window says so
and offers **Walk over**. Right-clicking the desk itself gives the same
in one row — **Trade** walks your Bim over and opens the window. What you
buy still goes straight into the hold, wherever aboard it is kept; the
desk is where the deal is struck, not where the goods land. A buy or a
sell sent from across the room is refused with *nobody of yours is at
the trading desk*.

### What the crew can see

The crew see **all the way round** — there is no cone — and what stops
their eyes is what stands in the way: a wall, a bulkhead, the hull, a
shelf or a reactor, a door with its leaves shut. Not the low things: a
Bim sees over a table, a bunk, the worktop, a tray of crops. It is
**traced**, tile by tile, from every crew member's eyes, and it is
**shared**: what one of them sees, all of them see, and the screen shows
the crew's view.

A Bim standing **against a wall peeks round it**: as well as from where it
stands, it looks from the tile either side of it along the wall, and what
that adds is whatever lies past the wall's line. So a Bim pressed to the
corner of a room sees the whole of the room round the corner, where one
standing a tile back sees only the wedge the corner leaves — the wall's
edge cuts the view. A peeking Bim shoots from the peek.

What nobody sees is under **one fog, the same everywhere** — the way Dota
draws it. The whole map of your ship, a station, a town and the plain
round it is always drawn: the walls, the doors and the fixtures show
through the fog whoever the structure belongs to and whether or not
anybody has ever looked there. Nothing is black. What the fog hides is
**who is standing there**: your own crew are always drawn, and everybody
else — a station's people, the machines, the Manufacturers — appears only
in line of sight, and stays drawn for two seconds after it was last in
it, so somebody stepping behind a bulkhead is a moment fading rather than
winking out. What the crew see is always brighter than the fog: clear
under a lamp, a shade darker where no light reaches, and never as dark as
the fog. From outside, a station is its hull under the same fog, and
nobody in it is drawn. Home is the station you set out from, a station
the machines hold is hostile, and every other is neutral — which decides
who fights you, not what the fog looks like.

### The two views

**Ship** is the live ship at tile scale, drawn turned to its heading, with the
starfield behind it. The hull is plated, with running lights to port and
starboard and a strobe at the bow. A run's ship is never under way — it is
docked, landed or holding station — so its engines never burn and its
thrusters never puff: the exhaust the flown trip drew went with it
(feature 104). **System map** is the star, what the crew have found, the
ring the scanner reaches to, and a little hull pointing where the ship is
pointing.

The camera is **head up in both by default**: the ship held square to the
window, the deck the way it was laid out, and the sky and the map turned
round it instead. **North up** (`N`) is the other choice:
the camera never turns and it is the ship that turns on screen.

The ship view **follows the crew member you steer**: your Bim is what sits
in the middle, on the deck or across a station, and a drag can shove the
view only so far before it would be off the edge. **Free camera** (the View
tab, or `V` — `F` is the attack order since feature 84) lets it go — the view stays where it is and a drag or the
keys take it anywhere, for looking at the far end of a station while the
crew are busy at this one — and **Follow** snaps it back.

### Saving, and picking it up again

Esc opens the settings, and **Save** and **Load** are on it. A save is the
whole game as it stands — the clock, the ship and where it is, the room
aboard with everybody in it mid-errand, the stations and what is on their
shelves, the hold, the money — written as text to
`~/.local/share/bims/saves/<name>.ron` (`$XDG_DATA_HOME`, or wherever
`BIMS_SAVES_DIR` points). A name is letters, digits, `-` and `_`; a name
already used is written over, and the page lists what is there to pick
from. Load lists the same files, newest first, and puts the game back
exactly where it was: the crew carry on from the step they were at, and
the checksum of the world read back is the checksum of the world written.
What is *not* saved is the window's — which panel is open, where the
camera is, what you were aiming at — so a loaded game opens on the whole
ship the way a new one does.

The menu at the start has a **Load** button too, beside Play, so a game is
picked up without walking through the setup and the yard. There is nothing
to save in the yard: the game starts when the ship is accepted, and the
button says so until then. A file written by another version of the game
is refused by its version rather than read wrong.

### The two crates behind that

- **`crates/flight`** is what a design does when you push it — mass, centre of
  mass, inertia, acceleration, how fast it turns. It also has the closed-form
  plan the flown trip was read off, worked out **once** and read at a time so
  that nothing integrated; a run flies nothing, and its trips are quoted off
  the ship's accelerations instead (`physics::travel_days`, in *The loop*).
- **`crates/world`** is the star system, the ship in it, the run, the clocks,
  and the order things happen in. It renders nothing and knows nothing about
  a window.

What the steps after this one are promised is written down at the top of
`crates/world/src/lib.rs`: Bims live in ship-design tile coordinates and the
ship's rotation does not reach them, every ship change goes through
`on_ship_changed`, construction spends what is aboard and asks
`can_modify_part` first, and money is only for docks.

Which world you land in is a seed and a galaxy shape, and for now they come off
the settings the lobby hands over, with a fixed default behind them. The
lobby's World tab will pick them instead; nothing else about them changes.

## What the ship will make

None of this exists yet. It is the contract the next few steps are built to,
written down first so that each of them is measured against something, and
each heading below is replaced by a description of the real thing as it
lands. The order is the order of the headings: power, the resources, the
workstations, the walk outside and the armoury — all built, and described
under the designer and the game — then the fight, of which the machines'
half is built, the boarding half was built and deleted again, and the
rest is not.

### One mechanism, the tree, and the mass

Built, and then mostly **taken away again**. *Making things* under the
game is the mechanism and the table as they stand: one row, two
vegetables into a medkit at the drug lab. The tree of production the
mechanism was built for — ore into metal into components into emitters
into guns — went with the money rework: there are no materials, a part of
the ship costs euros, and a gun is bought rather than smelted. What the
mechanism kept is the mass rule, which still holds for the one recipe
there is: a medkit weighs exactly the two vegetables that went into it.

### Power

Built. It is *Power* under the designer and *The reactor, the batteries and
the brownout* under the game, below.

### The walk outside

Built, and then emptied: *Mining, on foot* under the game says what is
left of it. The suit and the airlock are still how a construction site
beyond the hull is reached; there is nothing out there to dig.

### The armoury

> **Since task 113 the armoury is furniture**: the guns and the armour
> nobody wears are in [the Armory](#the-armory), which is no place on the
> deck. What follows is how it used to work.

Built, and then **stripped back to a cabinet**. The **armoury** used to
be a bench and a locker in one, making the laser handgun, the vest and
the four other weapons out of metal, components and emitters. It makes
nothing now: since the money rework a weapon is **bought** at a desk that
has the weapon trade (see *Trading*), and the armoury is what it always
also was — the locker-class cabinet a crew fetch a gun out of and stow
one into, eighty cells of it. The **vest** is gone entirely; the three
real pieces of armour replaced it. Beside it stands the **drug lab**, the
one bench that still makes anything: two **vegetables** into a
**medkit** in a quarter of an hour. A medkit is what gets a Bim out of a
dying state and a **bandage** — bought, not made — is what closes the
wounds on one part of a body; see *Getting hurt* under the game. What a
Bim does with a handgun is the fight, below.

### Armour

> **Since October 2026 a Bim wears one armour**, over the whole body:
> no helm, no leg guards, no arc greaves or Reflective plate. It comes
> at three tiers; tier one is the three old pieces put together — 45
> health, 1.8 protection — and every tier up is half as much again. A
> hit lands on the body and nowhere in particular. The Tab panel shows
> two slots a Bim: the weapon and the armour.

> **Since task 113** a piece is never destroyed and never lost: at
> nothing it stays worn, doing nothing, until the next mission makes it
> whole; and it moves only on [the Armory](#the-armory), between missions.
> The hold, the pack and the workbench below are gone.

Built. Three pieces — a **helm** (+15 health, 2 protection), **kevlar**
(+20, 2) and **leg guards** (+10, 1) — **bought** rather than made, at
any place with the armour trade, priced at **a hundred euros a point of
health**: leg guards 1 000, a helm 1 500, kevlar 2 000. The playtest ship
carries one of each. A
piece is two things at once, on purpose. **In a container it is a
resource** — `Helm`, `Kevlar`, `LegGuard`, locker class beside the medkits
— so buying, selling, mass and the shelves work on it with no
new mechanism; **anywhere else it is an instance**, with an id that only
climbs and a health it keeps wherever it goes. The world keeps every piece
there is (`World::pieces`: id, kind, health left, and where — the hold, a
pack cell, or worn by somebody) and holds one invariant against the hold:
the count of each armour resource is always the number of pieces in the
hold of that kind. A purchase or a workbench upgrade pushes a whole piece;
a sale takes the most damaged one first.
What is worn and what is in the pack are the
room's (`bims::combat::Gear`), since the room's health reads them, and the
world reads them back every step for the checksum.

Five commands move a piece about, the way a craft target is a command —
the hold is the world's, and every player's ship has to agree what is in
it: **Fetch** takes a piece (or a unit of anything) out of a container into
the crew member's pack, **Stow** puts one back, **Equip** puts on what
is in a pack cell and swaps what was worn into it, **Unequip** takes a
piece off into the pack, and **Discard** throws one away — and a sixth,
**Loot**, takes one thing off a body that is down (see [Combat
mode](#combat-mode-and-the-inventory)). A fetch or a
stow wants the crew member within two tiles (`REACH`) of a container that
takes the thing — a shelf, the armoury or the drug lab for locker goods,
the cold store for food —
and is refused *out of reach* otherwise; a full pack refuses a fetch, a
full class refuses a stow, and a **broken** piece — at nought — cannot be
stowed or sold at all, only discarded. Equipping wants no container. What
a worn piece does to a hit is [Getting hurt](#getting-hurt): the
protection comes off the damage first, what is left drains the piece, and
only what the piece could not take reaches the body.

**Every weapon and every piece has a tier**, one to three. A tier is
**bought** — any market that deals in gear deals in every tier of it, at
the book times one, four and sixteen — or **made at the workbench**, from
the first day (research, which once gated it, is gone — feature 106).
Two of a kind at
the same tier go into
its two slots, **Upgrade** in its window starts a day of work on them,
and one of the next tier comes out in the third slot — a quarter more
damage and accuracy for a weapon, half again the health and protection
for armour, and at tier three a little more range or a chance to dodge.
A tier-two thing lies on a blue cell wherever it is drawn — a locker, a
pack, a body being looted, the bench — and a tier-three on gold, so an
inventory says at a glance what is worth taking. A crew member carries
a thing to the bench by hand (Ctrl-click it in the pack with the bench's
window up) and takes the result off the same way; with **Combine
matching gear** ticked on the Management tab the crew do it themselves,
one thing in the arms at a time, from the lockers to the bench and back.

### The fight

- **Between ships**, turrets fire on a plan-shaped clock rather than an
  integrated one, damage lands on a tile, and a **destroyed part leaves
  scrap, not its recipe**. That is the one place the materials contract's
  "no loss" is broken, and it is broken on purpose and named. Outside walls
  have hit points; a shield trades power for damage. Shields, turrets or a
  full burn off one battery is the decision the power system was built for.
- **Salvage** is docking at a derelict and taking its parts apart with a Bim
  in the middle — the caller `materials.rs` says the deconstruction rule is
  waiting for. What a derelict has left in it is generated already.
- **Boarding** was docking to somebody hostile — a station whose people
  were the enemy, three in ten of them as the generator rolled it, or a
  raider tied up to the ship — with the rooms joined into one, one nav
  grid, and Bims with handguns in the corridors. It was built, both ways,
  and it is gone: every enemy is a machine since the first step of the
  roguelike redesign, and the third deleted the human ones — the hostile
  stations, the raiders, their garrisons and their plunder (feature 104).
  What it was built on is all still there: the rooms joined, the
  corridors, the cover, and a hit that is a wound on a part of the body
  bleeding until somebody dresses it — see [Combat
  mode](#combat-mode-and-the-inventory). Armour is in too — the section
  above.

- **The machines** are the endgame enemy, and they are in as far as a
  probe goes: a race of three — the low, quick **Husk** that scuttles on
  four legs and snaps its claws at arm's length; the **Trooper**, upright
  with a gun built into its forearm, which walks into the open and fires
  on the move; and the broad, shoulder-plated **Warden**, whose
  **Unmaker** strips the armour off whatever it hits rather than opening
  the body under it. At an **elite** — and nowhere else — a wave has a
  fourth: the **Guardian**, the largest, a heavy walker
  behind a **shield** that stops every bolt and every blow coming at it
  from the front, the ±60° its plate covers, flaring where it stops one.
  The shield cannot be broken, but a grenade's burst is not stopped by
  it, and the arc is marked faintly on the deck under it: **get round
  it**. It turns to face the nearest crew member it sees no faster than 75° a second, and walks in the open,
  the shield being its cover. Its weapon is the **Sweeper**, a beam: when
  it has a crew member in front of it, it **plants its feet and winds
  up** for a second and a fifth — its heading fixed, so that is the
  moment to get round it — then **sweeps** the beam through twenty
  degrees across where it aimed in half a second, and rests three and a
  half before the next. The beam reaches twenty tiles, stops at the
  first wall or shut door, and goes **through** bodies: everybody in
  its arc takes thirty, once a sweep. Sandbags between you and it are
  cover — the beam goes over — unless you are leaning out of them, and
  a tank's wall and a relic's surge work on it as on a bolt. It hurts
  no machine. **Come within two tiles of it and it drops a grenade at
  its feet** — once every ten seconds — that bursts a second and a half
  later two and a half tiles wide, half again the beam's damage at the
  middle and half that at the edge, on the crew alone: step back out of
  it. An elite's second wave holds one Guardian at tier one, two at tier
  two and three at tier three; no other wave has any.
  `nix run .#guardian` is the fight against one. A machine is not a Bim: it has four parts rather
  than three (head, chassis, arms, legs), no blood, no dying state — head
  or chassis at nothing and it is a wreck that instant — and nothing to
  loot, since its arm is part of it. Shoot its arms off and it aims half
  as well; shoot its legs off and it fights where it stands — a Husk,
  which has only its claws, drags itself on at a third of its pace.
  A **droid-held station** has no people at all, and its machines come in
  **waves**: how many waves is fixed the first time the crew dock there,
  how big each is worked out as it lands — off the world clock and the
  number of players, and nothing else — and the next never arrives
  while one of the last is still standing — two hours of the mission clock
  after the last one falls, a reinforcement ship tying up at the far airlock or
  a lander coming down on the plain beyond a town's gate.
  `nix run .#droids` and `.#droids_planet` are how one station's fight is
  looked at on its own.

- **The crisis** is which stations they hold, and it is a clock rather
  than a dice roll. Every galaxy is webbed with **hyperlanes** — each
  star joined to its three nearest neighbours, and whatever else it takes
  to leave one connected graph, drawn faintly under the stars on the
  galaxy chart. **From day nought** the machines hold **one star**,
  rolled at the start of the game at least eight lane hops from wherever
  the crew began, and from there the infestation spreads **one hop every
  five days**. That is the whole rule: a star is theirs from
  `5 × hops` days onwards, so two players in one game agree about the
  whole galaxy without a word passing between them, and the chart can say
  under any star you pick exactly which day it is due. At three lanes a
  star a galaxy runs a hundred-odd hops across, so the last star in it
  falls somewhere around **day 480 to 715** — the crisis is a season, not
  a raid. (It used to hold off until day ten; since feature 102 it is
  there when the run opens, and a system due by then is the machines'
  whole the moment you arrive in it.)

  **An infested system is the machines'**: every station in it, orbital
  or town, its people gone, nothing to trade, nobody to hire, no shelf to
  buy from — and a wave of machines standing in each. A station the crew
  **cleared** stays cleared: the crisis never re-arms it. Since only travel
  moves the clock, a system's day never comes while the crew are in it: it
  comes on the way, and the crew arrive to find the place the machines'.
  `nix run .#crisis` opens the simulation on the eve of it.

- **The jammer** is what makes an infested system a corner rather than a
  place to pass through. Every one of them has exactly one — the orbital
  station with the lowest number, or, where a system has no orbital
  station at all, one the machines built themselves out in the dark — and
  while it stands, **the lanes inward are shut**: a jump to a star
  *nearer* the machines' origin than this one is refused. Sideways and
  outward are open, and a trip *into* an infested system is never
  refused. So pushing towards where the machines began is a one-way door
  each time, and the way back opens only by boarding the jammer station
  and destroying every wave on it — once cleared it stays cleared, through
  a save, a load and every later spread. The chart says which station
  holds it and bars in red every step of a route a jammer would turn
  back, and the world map's list says which place is a system's jammer.

  **And the machines get harder the nearer their origin you are**: within
  two hops of it every wave comes at **tier three**, with tier-three arms
  and tier-three armour, where the rest of the galaxy meets them at tier
  one.
  `nix run .#jammer` opens the crew inside an infested system two hops
  from the origin, with the jammer standing and a wave aboard.

- **The front** is the ring of systems just outside the infection, and it
  is a place to trade in rather than a warning to read. A system three
  hops or fewer from the edge is on it, and the nearer it is the more it
  shows: **weapons, armour, medkits and bandages** cost and fetch fifteen
  per cent over the odds on the edge itself, ten two hops out and five
  three hops out, while the food is priced as it is anywhere: a crew
  buying its guns a hop from the machines pays over the odds for them,
  and one selling its kit there is paid over the odds. The trade window says
  so outright when you are at such a desk. And there is **one more hand
  for hire** at every station on the front — people are on the move where
  the machines are coming, and armed.

- **Defending a town** — since task 111 every place that is neither a
  trader nor an enemy's, from the first day ([Attack, defend or
  trade](#attack-defend-or-trade) has the whole rule). Set down at a
  town and the machines come for it: twenty seconds later a wave lands
  outside a gate and walks in, and another after each is destroyed. It is
  the one fight with somebody else on your side — the town's **guard** and
  whatever **mercenaries** live there take arms and fight beside you,
  everybody else walks into the nearest house and stays there, and the
  machines come for the townsfolk as readily as for you. **Go back to the
  ship before the last wave is down and the town falls to them.** Let the
  last wave be destroyed and the town is **held**: it stays friendly for
  good — trading and hiring even after its own system falls to the
  crisis, the one friendly desk left inside it — and **some of its people
  join your crew**, a fifth of the survivors, never the guard, as
  classless hands who draw no wages. Let every one of its people die and
  it falls like any other place.
  `nix run .#defense` opens a town with the machines a minute away.

### Classes and levels

Each player picks a **class** for their crew member —
**Engineer**, **Soldier** (the one it starts on), **Medic**, **Tank** or **Commander**; there is no classless pick — on the setup tab before Start (the lobby
deals it with the slot, like the hair), and can still change it on the
crew panel until the ship first leaves the place the run opened at. A crew member has one
class, and a dead one's level, experience and picks die with it.

**A class is worn**, so the deck says who is who at a glance: the
coverall is dyed the class's own shade — an engineer ochre, a soldier
olive, a medic white, a tank steel, a commander navy, and a crew
member with no class the blue it always was — with something on the
head and something over the chest to go with it. The engineer is in a
hard hat with a tool belt and pouches; the **soldier** wears
sunglasses, crossed webbing with three magazines on it and a belt; the
medic a capped red cross with another on the chest and a bag on the
hip; the tank a heavy helm with a pauldron on each shoulder, and the
biggest body of the six; the commander an officer's cap banded in
gold, gold shoulder boards and a sash. The coverall underneath still
says whose it is — an engineer of the crew is an ochre-ish blue where
one ashore is an ochre-ish orange — and a helm worn over the class's
own cap carries a band of its colour instead, so a helmeted crew are
still read a class at a time. The setup tab's portrait wears whatever
class is picked under it.

**A class owns abilities, never jobs or money.** Every crew member does
every job, takes every errand, places every site and uses every weapon,
and every one brings the same money to the pool, whatever its class. A
class only adds its own two keys and its talents, which nobody else can
use: **Q** is the class's first action and **E** its second, whichever
class you steer — an engineer's mine and satchel charge, a soldier's grenade
and Stun Shot, a medic's Heal Drone and heal beam, a tank's Riot Shield and
Reflect Barrier, a commander's rally and attack order — and both are rebindable on
the Controls page as one pair,
not one a class. They are the first and third of **four ability slots**,
laid out **Q E F Space** (task 123; the second slot was on **C** until
October 2026, when C became the ping, and so it sits third, on **F**),
and **Ctrl** with a slot's key, or a Ctrl-click on its box, ranks that
ability up instead of using it. The **commander** has two keys more, his alone: **X**
calls the squad back and **Z** has it stand its ground, and both do
nothing for any other class. The **medic** has one of his own: **G**
picks a crewmate up and sets them down. With a classless crew member
steered the keys do
nothing; a press the world refuses says why in the log.

**Every ability has a box at the foot of the screen**, one a key: the
key in its corner, the picture in the middle, how many are left in the
other, and the name under it. Every class has the four slots' boxes, Q C
E R, the second and fourth empty frames for now (task 123); past them a
commander has two more — fall back, stand ground — and a medic one, the
carry. **Resting on a box rings
the Bims that cast would reach** on the deck: the crew a rally would
lift, the squad each of the three squad orders commands, the patients a
beam holds, whoever a carry could pick up. It is the panels' own rule —
resting on a row rings what it names — asked of an ability instead of a
fixture, and it is how you see what an order is about *before* you give
it. While a rally is actually running, the Bims it lifts wear its own
chevron rather than the aura's plain ring, so a rally called is told
from an aura standing there all along.

**Experience is the same for every class** (task 119), and comes from
two things and nothing else, to every classed crew member within fifty
tiles: an enemy **going down** is 10 — out cold, or dead without being
down first, as every machine is — and an enemy **dying** is 5 more,
whether it died where it fell or bled out later; so a machine destroyed
is 15. Each enemy counts once for each; a crewmate or a hire going down
is nothing. Nothing a class does — building, laying a kit, bandaging or
treating, being shot at, hiring — is worth anything. Ten levels — 100, 250, 450, 700, 1 000,
1 400, 1 900, 2 500 and 3 200 for the second to the tenth — the same
shape for every class: the first, third and seventh are fixed, and every
other level is a **pick of two talents**, never changed once made.

**Twenty levels now (October 2026).** Every class climbs the same table
— 3 200 experience for the sixteenth, then 3 570, 3 980, 4 430 and
**4 920 for the twentieth** — and the skill points stop at the sixteenth,
where every rank of the four abilities is bought. Each of the four
levels past it is **ten hit points** on the bar as every level is (300
at the twentieth) and **five per cent more weapon damage** — a gun's or
a blade's, never an ability's — so a twentieth-level Bim hits a fifth
harder. The log says it: *reached level 18 — weapon damage +10%.*

**A level is spent on the character sheet** (**K**, feature 107; it was
the tray's Skills tab). The class's ten levels are a tree
on the sheet, numbered down the left: a level
with nothing to choose is one slot across the width, a pick level is two
side by side, and over the tree is how many **skill points** are waiting
— one for every level reached that has not been chosen at. A slot
learnt is filled in, one given up is struck through, one open is ringed,
and one at a level you have not reached is dark. Click a slot and the
lines under the tree say what it does — and **what it is worth in
numbers**: what the figure is now and what it becomes, a sentry's 60
health to 90, a heal beam's 6 tiles to 9, a grenade's 2-second fuse to
1, so the choice between a level's two slots is a choice between two
numbers. A slot that is open has the
*Learn* button on it, which spends the point and cannot be undone.
While a point is waiting the hero panel carries a **+1**, which opens the
sheet; nothing opens by itself, nothing is learnt until it is spent, and
the choice waits as long as you like.

**What the two keys do is drawn on the hero panel**: a box each
for **Q** and **E**, with the key in one corner, the picture of what it
does in the middle, **how many are left** in the other — kits and
grenades in the pack, sentries the talents allow standing, beams free to
link, the squad's size. Resting on a box names it and says
what the key does. A box is lit while the key would be taken and dim
while it would not: the cooldown counts down over the picture, and a
key not learnt yet says the
level it is learnt at — every class's **E** from the first level and its
**Q** from the third. A classless crew member has no keys and no boxes.

### The engineer

The first class (feature 74), reworked in task 127 into **four
abilities, four ranks each**, the soldier's way: **sixteen levels**, one
**skill point a level**, a rank bought with **Ctrl** and the ability's
key, a **Ctrl-click** on its box, or the Skills tab's **Learn** button.
Q, C and E take a rank at levels 1, 3, 5 and 7; Space, the ultimate, at 6,
9, 12 and 15. An engineer at rank nought of an ability has none of it.

**Charges, never kits.** The mines, the Healing Sentry and the satchel
charges are **charges**: a count the world keeps for the engineer, set
by the ranks, and each spent one coming back on its own cooldown.
Nothing is crafted, fetched or carried, and no kit lies in anybody's
pack. Laying takes the charge only when the work is done; a hit on the
engineer while it lays a mine or a Healing Sentry stops the laying and
the charge is kept. The **Nearby** strip's row beside a mine or a
Healing Sentry **packs it up** — the charge back, never more than its
charges. Nothing laid is anybody's experience. The sandbags and the EMP
were the engineer's E and Q until task 154.

**Q, Mine.** Laid on the deck tile under the pointer in a second. It
lies there, small and dark with a red light, until an **enemy** — a
machine or one of the Manufacturers, never the crew — comes **within a
tile** of it; then it goes off: every enemy in its blast with a clear
line takes its damage, full at the centre and half at the edge, halved
again in cover. It never touches the crew or their sentries. Lay one
past the standing limit and the oldest is taken up.

| rank | damage | blast | charges | laid at once | back after |
| --- | --- | --- | --- | --- | --- |
| 1 | 40 | 1.5 tiles | 1 | 4 | 25 s |
| 2 | 50 | 1.5 tiles | 1 | 6 | 22 s |
| 3 | 60 | 2 tiles | 2 | 6 | 20 s |
| 4 | 75 | 2 tiles | 2 | 8 | 18 s |

**C, Healing Sentry.** A sentry with no barrel, laid on a deck tile:
every step it heals every crewmate on their feet within its radius and
in its sight, up to a full bar, at a share of the medic's beam — a thin
green line runs to each it heals. It never heals an enemy and never
revives a downed crewmate; two reaching one crewmate do not add up, the
better one counts. The enemy shoots at it like a sentry. Its charges are
how many may stand: one more laid takes down the oldest. Its health was
doubled in task 154.

| rank | heals (× the beam) | radius | health | laying | charges | back after |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | ×0.5 | 3 tiles | 120 | 6 min | 1 | 60 s |
| 2 | ×0.75 | 4 tiles | 160 | 6 min | 1 | 60 s |
| 3 | ×1 | 4 tiles | 200 | 4 min | 1 | 50 s |
| 4 | ×1.25 | 5 tiles | 240 | 4 min | 1 | 40 s |

**E, Satchel Charge.** Thrown like a grenade — a **quick throw**:
holding `E` draws the reach (8 tiles) and the blast's ring, letting it
go throws at the pointer, walking out first where it must. Two
charges at every rank. A satchel lies where it lands — **several may lie
on one tile**, stacked — until the **remote trigger, G** (Space until October 2026), sets off
every satchel of yours at once, each its own blast on the enemy alone:
three stacked on a tile hit three times. Nothing else sets one off, and
it is never packed up.

| rank | damage | blast | charges | back after |
| --- | --- | --- | --- | --- |
| 1 | 35 | 2 tiles | 2 | 30 s |
| 2 | 45 | 2.5 tiles | 2 | 27 s |
| 3 | 60 | 2.5 tiles | 2 | 24 s |
| 4 | 85 | 3 tiles | 2 | 20 s |

**Space, Sentry (the ultimate).** A minigun on a stand, drawn larger. The
engineer works three minutes beside the tile and **a hit does not stop
it**. One stands at a time (two with an Override Core); it fires at the
nearest enemy it can see, five tiles further than a minigun, the enemy
shoot at it, and it stands until it is shot to pieces or the ship leaves
the site. It cannot be packed up. The cooldown starts when it is laid,
runs on the mission clock, and it is ready at every mission's start. Its
health was doubled in task 154, when the sandbags went.

| rank | weapon | fire rate | health | cooldown |
| --- | --- | --- | --- | --- |
| 1 | minigun, tier 2 | ×1 | 400 | 150 s |
| 2 | minigun, tier 3 | ×1 | 500 | 140 s |
| 3 | minigun, tier 3 | ×1.5 | 600 | 130 s |
| 4 | minigun, tier 3 | ×2 | 800 | 120 s |

On the ship's deck mines, satchels and a Healing Sentry stay wherever
the ship goes; on a station's deck they are lost when the ship leaves.

### The soldier

The second class (feature 75), reworked in task 124 into **four
abilities, four ranks each**, the way Dota does it. A soldier sets out
with a basic **auto rifle** in hand and the laser pistol in the armory.
It climbs **twenty levels** (3 200 experience for the sixteenth, 4 920
for the twentieth) and earns **one skill point a level** up to the
sixteenth, the first level included; the four after it are weapon
damage (see *Classes and levels*). A point buys a **rank** of one ability: **Ctrl**
and the ability's key, a **Ctrl-click** on its box, or the **Learn**
button on the character sheet's Skills tab. Q, C and E take a rank at
levels 1, 3, 5 and 7; Space, the ultimate, at 6, 9, 12 and 15. A point you
do not spend is kept. The boxes on the hero panel show a pip a rank
under each, a **+** in the corner while a point could buy the next, and
the points waiting sit beside the experience bar.

**Q — Frag Grenade** (active, charges). Thrown at the deck tile under
the pointer, within **8 tiles** with nothing opaque between (walls and
shut doors stop a throw; sandbags do not): a **quick throw** — holding
`Q` draws the reach and the burst's radius, letting it go throws at the
pointer (a click throws sooner; a right-click or `Esc` takes it back; the
engineer's satchel on `E` is the same). **2 seconds** later it bursts on everything within the radius
with a line to it — every crew member, hire and enemy alike, the thrower
included — full damage at the centre falling to half at the edge, halved
again in cover from the burst's side; a sentry in it takes the same. A grenade is a **charge**: it comes back on
its own cooldown, and nobody makes, sells or buys one.

| rank | damage | radius | charges | cooldown a charge |
| --- | --- | --- | --- | --- |
| 1 | 60 | 2.0 tiles | 1 | 30 s |
| 2 | 75 | 2.5 tiles | 2 | 30 s |
| 3 | 90 | 2.5 tiles | 2 | 24 s |
| 4 | 110 | 3.0 tiles | 2 | 20 s |

**C — Weak Spot** (passive; pressing `C` does nothing). Every bolt and
every blow you land on an enemy may strike a weak spot — each bolt of a
burst on its own, a grenade never. A critical hit adds the weapon's own
damage at that distance times the crit damage less one, after every
relic's bonus and before the armour, so a relic's share is never
multiplied by it. A critical hit's flare is twice the size and the
brightness of an ordinary one.

| rank | crit chance | crit damage |
| --- | --- | --- |
| 1 | 10% | 150% |
| 2 | 12% | 175% |
| 3 | 15% | 200% |
| 4 | 20% | 225% |

**E — Stun Shot** (active, cooldown; it was Brace until October 2026).
It is aimed as the grenade is: holding `E` draws the weapon's reach round
him and the burst's ring where the shot would come down, under the
pointer; letting go (or a left click) charges the shot at that spot.
He charges it for two seconds,
holding his fire — the rifle up and glowing at the muzzle, a ring
closing on him, the burst's ring kept on the spot — and **walks wherever
he likes while it charges** (keys, an order, a roll), **facing the spot**
whatever the pointer says. Then it fires at
the spot from where he then stands: a fast slug that **bursts on the
first enemy in its way**, else where it lands, **never past his weapon's
reach** and stopped short of the first
wall, as wide as his grenade's burst. Every enemy in it with nothing
opaque between takes its damage and is **stunned for three seconds** —
every machine, the Machine Heart's too (a stunned fabricator skips its
build, a stunned core's beams go to their cooldown), and the
Manufacturers' people, who neither walk nor fire while it lasts; the
crew are never hurt. Going down calls the charge off, and the cooldown
runs only from a shot fired.

| rank | damage | radius | stun | cooldown |
| --- | --- | --- | --- | --- |
| 1 | 15 | 2.0 tiles | 3 s | 30 s |
| 2 | 20 | 2.5 tiles | 3 s | 27 s |
| 3 | 25 | 2.5 tiles | 3 s | 24 s |
| 4 | 30 | 3.0 tiles | 3 s | 20 s |

**Space — Rampage** (ultimate, active). For its seconds the soldier fires
faster, takes less, and aims on the move as well as standing still. It
is ready at the start of every mission, runs on the mission clock (a
pause stops it), may be used with a Stun Shot charging, and the cooldown
relics (*Hair Trigger*, *Overclocked Cores*) and the *Coolant Loop* item
move its cooldown as they do every class ability's. Its box glows while it runs.

| rank | duration | fire rate | damage taken | cooldown |
| --- | --- | --- | --- | --- |
| 1 | 8 s | ×1.5 | ×0.80 | 35 s |
| 2 | 10 s | ×1.75 | ×0.75 | 30 s |
| 3 | 12 s | ×2.0 | ×0.70 | 25 s |
| 4 | 12 s, +1 s for each machine you down during it, +6 s at most | ×2.0 | ×0.70 | 20 s |

### The medic

The third class (feature 76), a **ranked kit** since task 130 and
**reworked by task 153**: four abilities, **four ranks each, bought with
a skill point a level**, the soldier's way — sixteen levels, Q, C and E
ranking up at levels 1, 3, 5 and 7 and the ultimate Space at 6, 9, 12 and
15. A medic sets out with the **laser pistol** in hand as everybody
does. Its experience is everybody's: healing earns nothing (task 119).

**Two base traits**, whatever his ranks: he **revives in four seconds**
where anybody else takes ten, and the crewmate he gets up stands at
**40%** of its bar where anybody else's gets up at 30%.

**Every heal he gives** — the link, the drone, the circle — is lifted by
his **Triage** on the Bim healed, and by half as much again (**×1.5**)
while he carries an **Override Core**: that item's gift to a medic,
whatever rank his circle is at. A revive is no heal and takes neither.

**Q, Heal Drone** (active, cooldown): a little drone dropped at his
feet flies — **over walls**, it flies — to the friendly Bim on its feet
**lowest on its bar** (by share; the medic himself counts), hovers over
it and heals it slowly. When that one is whole, down or gone it picks
the next lowest; with nobody hurt it keeps by him. **A site's defenders
count too, crewmates first**: it goes to a hurt defender only while no
crew member is hurt, and leaves one for a crewmate the moment one is.
One drone a medic.
It is drawn as a small quadcopter with a green cross, a thin line down
to the Bim it heals and a ring round it emptying as its time runs out.

| rank | heal a second | lasts | cooldown |
| --- | --- | --- | --- |
| 1 | 1.8 HP | 8 s | 25 s |
| 2 | 2 HP | 10 s | 22 s |
| 3 | 2.5 HP | 12 s | 20 s |
| 4 | 3 HP | 14 s | 18 s |

**C, Triage** (passive): his heals are stronger **the less health their
target has left** — the full bonus on a Bim near nothing, half of it at
half a bar, none on a whole one.

| rank | at an empty bar | at half a bar |
| --- | --- | --- |
| 1 | ×1.25 | ×1.125 |
| 2 | ×1.40 | ×1.20 |
| 3 | ×1.55 | ×1.275 |
| 4 | ×1.70 | ×1.35 |

**E, Heal Beam** — the **link** (toggle), on the crew member under the
pointer — a player's Bim, a bot, a mercenary, **a site's defender** (a
crewmate under the pointer first), or **the medic itself** —
never an enemy, within its range and in the medic's line of sight.
**A near miss still links**: off everybody, the key takes the nearest
friendly the beam reaches within three tiles of the pointer.
Pressed on the one it already holds, or on nothing, it unlinks. **He
keeps shooting at his full rate while linked**, and **the link heals him
as well** — as much as it gives a patient, once however many he holds,
and once when the patient is himself. A downed patient gets nothing
back: it wants reviving. The beam breaks when the patient leaves the
range (a wall coming between them does not break it), dies or leaves the room; when the medic goes
down, is ordered to an errand — a plain walk keeps it — or unlinks. A
line in the beam's green is drawn between the two on the deck (a ring
round a medic beaming itself), and the crew panel says who is held.

| rank | a second at 1× | range | patients |
| --- | --- | --- | --- |
| 1 | 2 HP | 6 tiles | 1 |
| 2 | 3 HP | 7 tiles | 1 |
| 3 | 4 HP | 8 tiles | 1 |
| 4 | 5 HP, plus what his items regenerate him by | 9 tiles | 2, each at the full rate |

**Space, Healing Circle** (the ultimate, a **toggle**): switched on, every
friendly Bim on its feet within its radius of him and in his sight
(walls block) — not himself — heals at **the link's rate**, his E rank's
(the first's before he has one), lifted by his Triage. **He pays for
it**: he loses as much health as a Bim in it is healed for, every second
it is on, whoever stands in it — leave it on too long and it **downs
him**, which switches it off. Every enemy standing in it **burns at half
the rate**, a pulse every half second. So a medic at E rank two heals
the crew round him 3 HP a second, loses 3 a second himself and burns
every enemy in it for 1.5. His link on himself, or on a crewmate, heals
him back: that is how a medic holds his circle up for long. On the deck
it is a soft green floor with a turning ring of light at its rim, motes
welling up all over it, the mending rising off every Bim it heals and
embers off every enemy it burns.

| rank | radius |
| --- | --- |
| 1 | 3 tiles |
| 2 | 3.5 tiles |
| 3 | 4 tiles |
| 4 | 4.5 tiles |

The Heal Drone is ready at every mission's start, runs on the mission
clock (stopped while paused) and is shortened by the cooldown relics like
every class cooldown; every mission starts with the circle off.

**The carry** is `H` (it was `G` until the held revive took that key), or the *Carry* row of the right-click menu on a downed crewmate, which walks the medic over first, from the first level and with no rank behind it
(feature 86): a **downed** crewmate under the pointer picked up into the
medic's arms and carried out of the fire. With the pointer on nobody it
takes up the nearest it could. Carrying, the medic walks at **six
tenths** of its pace and **fires nothing**. `H` again sets the body down
where it stands, and reviving it there is what comes next. The carry is
let go the moment either of the two goes down.

### The tank

The fourth class (feature 77), a **ranked kit** since task 139 and
reworked by task 155: a shield that bounces the enemy's shots back at
them, a barrier that hits back whoever hits him, and a shield thrown
over the whole crew. A tank sets out with the laser pistol everybody
does and a tier-one **armour** on — his kit comes with him and costs
the hold nothing. His experience is everybody's (task 119).

**His armour drains at half rate.** A piece's protection comes off a hit
as it does for anybody, and what gets past it drains the piece at half
the rate, so the same armour absorbs twice as much on him before it
breaks. The piece's stored health is never doubled: it moves between
crew members unchanged.

**Q, Riot Shield** (toggle): a flat plate of cold light held up in front
of him, facing wherever he faces. A shot that meets it **from the
front** is stopped there — nothing reaches him — its damage comes off
the shield's hit points, and it is **bounced back**: it leaves the plate
as his own shot, at the angle it came in (head on, straight back at the
shooter), and hurts whatever enemy it reaches. From the side or behind
the shield is nothing. It mends by its rank while put away, and while
held up **5 seconds** after the last hit on it; at nothing it **breaks**,
goes down, and cannot be raised again for a **10-second cooldown**
(shorter with the cooldown relics and items), mending all the while. It
flickers when it is low, and goes down with him.

| rank | hit points | mends |
| --- | --- | --- |
| 1 | 20 | 0.5 a second |
| 2 | 40 | 1 a second |
| 3 | 80 | 1.5 a second |
| 4 | 100 | 2 a second |

**C, Plated** (passive): every hit on him is cut before the armour takes
its share, and his health mends all the time.

| rank | damage taken | mends | extra |
| --- | --- | --- | --- |
| 1 | ×0.90 | 0.2 hp/s | — |
| 2 | ×0.85 | 0.8 hp/s | — |
| 3 | ×0.80 | 1.4 hp/s | — |
| 4 | ×0.75 | 2.0 hp/s | armour drain ×0.5 again, a quarter in all |

**E, Reflect Barrier** (active, cooldown): for its seconds every enemy
hit on him — a shot, a Sweeper's beam, a blow — is dealt back, as much
again, to whoever struck him, as his own hit. He still takes it. A
ring of amber thorns turns round him while it runs.

| rank | lasts | cooldown |
| --- | --- | --- |
| 1 | 3 s | 20 s |
| 2 | 4 s | 18 s |
| 3 | 5 s | 16 s |
| 4 | 6 s | 14 s |

**Space, Bastion** (ultimate, cooldown): every friend on their feet within
its radius — himself, the players and the bots — gets a shield of
**1000** hit points that **drains 100 a second**, so ten seconds at most;
a hit comes off it before anything else. With an **Override Core** (the
fifth rank) everybody it reached also walks **half again as fast** for
those ten seconds.

| rank | radius | cooldown |
| --- | --- | --- |
| 1 | 6 tiles | 70 s |
| 2 | 7 tiles | 60 s |
| 3 | 8 tiles | 50 s |
| 4 | 9 tiles | 40 s |

The cooldowns run on the mission clock, stop while paused, are ready at
every mission's start — the shield whole and down — and are shortened
by the cooldown relics and items as every class cooldown is.

### The commander

The fifth class (feature 78): everybody near him fights better, and the
crew nobody is steering take his orders. A commander sets out with the
laser pistol everybody does and nothing else. His experience is
everybody's: hiring earns nothing (task 119).

**He does two different things, and they reach different Bims.**

**His aura and his rally lift every friendly Bim near him**, a player's
own steered Bim as readily as a bot or a hired hand — never an enemy,
and never himself.

**His squad orders command only the squad**: every crew member **no
player is steering**, the crew's own bots and the hired hands alike.
They never move, hold or aim a Bim a player steers, and a Bim a player
starts steering leaves the order at once. Every player keeps every order
they have today: during the alarm anybody can still click a crewmate or
a mercenary and send it somewhere, and a click like that takes that one
out of the squad order until the next.

**The aura** is on from the first level and needs no key. While he is
conscious, every friendly Bim within **8 tiles** of him works a tenth
faster, shoots a tenth straighter, and holds its ground for a while
before it runs from a fight — where a dying body with no commander near
it runs at once. Two commanders' auras never stack: a Bim takes the
strongest one reaching it and no product of the two. A faint ring round
him shows how far it reaches and a small ring marks every Bim in it.

**Hiring is cheaper.** When the slot sending a hire is steering a
commander fit to act, the mercenary's fee is **a quarter off**, rounded
down to whole euros, and that is the fee written into the contract for
the whole engagement — it stays that hand's price whatever happens to
the commander afterwards. Dismissal still refunds nothing. The hire
window shows the discounted fee while a commander is steered.

**The three squad orders** work with the alarm and without it, so a crew
can fall back to the airlock before the machines are in sight. Each
reaches every squad member within **20 tiles** of him — the whole room
from the seventh level — and a member under one is in combat mode like a
recruited Bim: weapon drawn, errands stopped.

- **Attack** (`E`) on the enemy under the pointer: every squad member
  fires at that enemy ahead of any nearer target, and advances on it the
  way a crew member who has seen an enemy for itself does — to cover
  within range, peeking round it. The mark ends when that enemy is down
  or dead.
- **Fall back** (`X`) to the deck tile under the pointer, or to the
  commander himself when the pointer is on nothing: every squad member
  walks to a slot round that point — the same ring the crew gather in —
  holding its fire while it walks, then holds there and shoots what it
  can see.
- **Stand ground** (`Z`): every squad member holds exactly where it
  stands, shooting what it can see, never walking to cover and never
  running.

An order lasts until he gives another, gives the same one again (which
lets the squad go), goes down or dies; it is called off by the rooms
unjoining and by anything that moves the crew's indices — a hire, a
dismissal. A bracket over each squad member says it is under one, with a
thread to the enemy it was sent at or the tile it was called back to.

**Rally** is `Q`, from the third level. For **6 minutes** every friendly
Bim in his aura — a player's own included — shoots at ×1.3 and does not
run at all. It stacks with the aura; two rallies do not stack with each
other. The next rally waits **30 seconds** of the clock from the last,
and the crew panel counts the minutes left and then the cooldown.

| level | left | right |
| --- | --- | --- |
| 1 | the **aura**; hires at a quarter off; **Attack**, **Fall back**, **Stand ground** | — |
| 2 | **Wide presence** — aura radius ×1.5 | **Strong presence** — each aura bonus ×1.5 (a tenth becomes three twentieths) |
| 3 | **Rally** — may call it | — |
| 4 | **Haggler** — hires at two fifths off | **Outfitter** — a mercenary he hires arrives with one basic piece it lacked, free |
| 5 | **Focus fire** — the squad's odds against the enemy it attacks ×1.15 | **Pincer** — an attack may mark two enemies, the squad split between them |
| 6 | **Long rally** — a rally lasts ×1.5 | **Quick rally** — the rally cooldown ×0.5 |
| 7 | **Long reach** — a squad order reaches every squad member in the room | — |
| 8 | *Steady ranks* — no effect for now (task 120) | **Double time** — Bims in his aura walk at pace ×1.1 |
| 9 | **Relentless** — an attack's mark lasts until the enemy is dead, not merely down, and then the attack moves on to the enemy standing nearest the commander | *Grit* — no effect for now (task 120) |
| 10 | **Anchor** — the aura's bonuses double while he stands still | **Warcry** — a rally covers every friendly Bim in the room |

Every talent applies to the commander who holds it alone. The **aura and
rally talents reach every friendly Bim the aura or the rally reaches**,
a player's own steered Bim included; the **squad talents reach only the
squad**. *Outfitter*'s piece is the lowest basic one the mercenary is
missing — helm, then kevlar, then leg guards — made for it at the hire,
and the fee is not raised for it.

## The simulation

`nix run .#simulation` is the game without the front of it: it opens the
world at once, for one player, on the **playtest ship** —
`shipdesign::playtest_ship()`, which is the default ship every run sails,
a twenty-tile hull laid out with one of everything the old game's crew of
one needed to live and to fly. It is laid out the way a small ship would
be: a bow cut back to a point in diagonal hull, with the bridge in it —
the helm on the centreline, life support and a battery in the corners the
cut leaves; the main deck amidships, the galley along the bridge bulkhead
to port with the table under it, the bay in the middle, the bunk against
the starboard skin and the airlock behind it; and engineering aft — the
tank and the reactor down the port side, a shelf of stores, the heads,
and the main engine set into the stern so its bell is the stern. Three
compartments, two bulkheads with a two-tile doorway each, a run of conduit
from the reactor to the helm, and thrusters in the skins fore and aft.
The helm, the galley, the bunk, the bay and the heads are pictures now,
as they are on every ship. Every doorway and every gangway is two tiles
wide on purpose: the room's navigation cannot walk a one-tile gap. It has
`SIMULATION_MONEY` (a placeholder €50 000) in hand. It is for playtesting
the world quickly, and it is the one place the old "lowest star with a
station" spawn survives: the world is the fixed default seed's, a two-arm
spiral, docked at that system's first station. `nix run .#test` is much
the same somewhere else each time — the combat ship, which is the
playtest ship with bunks and chairs for five, a random seed, and a dock
somebody lives on picked at random across that galaxy.

The ship's part count and `design_hash` are pinned — `PLAYTEST_PARTS` and
`PLAYTEST_HASH` — and checked like the reference design's, so the simulation
opens on the same ship on every machine.

## The crew

Every player steers one Bim, and the rest of the crew are bots. Their
names are written over their heads on the deck and their portraits are
at the top left, and a crewmate you have selected has its health and its
crew sheet down the right — a click, a portrait or a drag to select,
right-click the deck to move, `l` to recruit, and the tray at the bottom
left ([The HUD](#the-hud)). The panels are one
module, `crates/app/src/crew.rs`. In the simulation the crew is one Bim,
James; in `droids` it is sixteen.

They are not two kinds of thing. A player's Bim and a bot are the same
body on the same clock, and nothing in the simulation tells them apart but
an index and whether anybody is steering it. The one asymmetry is the
player: **every order you give goes to the Bim you steer**. Selection, the
right-click move order, recruiting, and every row on every menu act on it
and only it: a right-click moves the Bim you steer whoever is selected,
and in a fight a bot cannot even be selected. The bots take orders two
ways only — **X** and **Y** are orders to every bot that follows you, and
a commander's squad orders move the bots nobody steers (*The commander*).

A Bim's name, coverall and hair are the app's business and the drawing's; the
simulation knows crew member 0 and crew member 1. No strings come out of the
room, so the names are written over the deck by the app after the shape
buffer has been replayed, not carried across in it.

### Picking one, and the crew sheet

**Clicking a Bim selects it, and selecting is what puts its panels on the
right-hand side**: its health, and a character sheet under it. One at a
time, and nothing at all when nothing is picked — the right-hand side
answers *who am I looking at*, not *what is everybody up to*. Your own
Bim, which is picked from the start, has no panel there: it is the hero
panel's and the character sheet's (feature 107).

**Selecting is looking at, not taking charge of.** Any of them can be
picked in peace, and only your own ever takes a right-click. Click a
crewmate, right-click the floor, and your own Bim goes — whoever is
selected. In a fight a bot cannot be picked at all.

The sheet is tabbed the way the tray at the bottom left is, because it is the
same kind of thing: pages of detail you open when you want them rather than a
readout to be watched.

- **About** — name, age, and the date they were born. Everyone aboard was born
  between **2350** and **2380**, rolled at the start of the game and fixed
  thereafter; the game opens on the first of January **2400**, so the crew are
  somewhere between twenty and fifty. The ship keeps a 365-day calendar with
  twelve months of the usual lengths and **no leap years** — a leap day buys
  nothing here and costs a special case in every piece of date arithmetic,
  including working out whether this year's birthday has been had yet.
- **Memory** — the Bim's own account of the crew it has lost.

### What a Bim remembers

**A crewmate's death, and nothing else.** Newest at the top, and a crew
that has lost nobody leaves the page **blank** — it says *"Nothing has
gone wrong."* and that is the good outcome rather than a panel that
failed to load.

It used to keep a great deal more: the day's work at first, then only
what went wrong — an accident, being sick, dropping off standing up, each
stage further into hunger or sleeplessness, the low moments of a Bim
nobody had spoken to — and what it saw happen to the others near it. All
of that was the needs', and went with them (feature 104). What is left is
the one entry that was always written into every surviving diary wherever
they happened to be standing, because it is the one thing aboard nobody
could fail to notice: *"Kate died today."* A Bim does not record its own
end; there would be nobody to read it back.

No strings come out of the room, here as everywhere. An entry is a day, a
time, a code and one number — which of the crew died — and `memory_line`
in `crates/app/src/names.rs` is where the sentence lives. The book is
bounded — a few hundred entries, oldest falling off the front — so a game
left running does not grow without end.

### They walk through each other

**Bodies do not collide.** Two Bims that meet pass straight through one
another, and both slow to **70% of their pace** for as long as they are within
a body's width. Close quarters are slow; that is the whole of the rule.

It is the one resolution that cannot leave anybody stuck, and being stuck is
the real hazard here. **A route is planned once and never replanned**, so a Bim
whose line goes through the other has no second plan to fall back on. Anything
that stops it getting where that line goes — pushing it aside, holding it up —
risks two of them standing nose to nose for ever with errands on both agendas.
Letting them overlap gives that failure nowhere to happen.

An earlier version did try to be clever about it: push them apart, pick one at
random to stand aside for three seconds, shove that one sideways out of the
other's lane. It worked, eventually, after two rounds of fixing what the fix
broke — but every part of it existed to stop bodies overlapping, and once
overlapping is allowed the whole apparatus has nothing left to do.

## Controls

| | |
| --- | --- |
| Click the armoury or a shelf, or pick the cold store on the **Nearby** strip | Its grid — the lockers, the shelves, the food — and the inventory of the Bim shown beside it; the Bim walks over |
| `Tab` | The inventory of the Bim you steer, and with it a body down within reach if there is one, with the rest of what is within reach a click away on the **Nearby** strip over it |
| Right-click a cell | **Take** · **Store** · **Equip** · **Discard**, or **Unequip** on a worn slot — greyed with the reason when it cannot go |
| Ctrl-click a cell | The quick move: container to pack, pack to the open container |
| Click or right-click a door | Menu: hold open, close, lock, unlock — the Bim walks to the panel |
| Right-click a Bim | A downed crewmate — a bot's or a player's: the menu with **Get up** (your own Bim walks over and revives it) and **Carry** (a medic walks over and picks it up), each greyed with the reason when it cannot. A bot already on its way gives way to you |
| `1` – `4` | Use the **item** in that slot of the Bim you steer at the pointer: a *Blink Drive* blinks there. The slots are the two-by-two grid at the right of the hero panel — see [Items](#items) |
| `h` | Swap the **medkit** and the weapon in your Bim's hands: with the medkit it holds its fire, and a right-click on a downed crewmate walks over and revives them. The hero panel lights the one in hand and says the key under them |
| `b` | A medic picks up the downed crewmate under the pointer (or sets down the one it carries) |
| Hold `t` | Standing close to a downed crewmate (two and a half tiles): your own Bim **gets the nearest back up** while the key is held, and stops if you let go first. A green bar over the body fills with the revive |
| `r` | **Reload** the gun of the Bim you steer now, shots left in the magazine or not (it was `t` until October 2026, when the ultimate took `g` — it is on **Space** now — and the revive `t`) |
| `F1` | Select the Bim you steer and put it in the middle of the view |
| `l` | Recruit it, or let it go — see below |
| `q` / `e` / `f` / **Space** | The steered crew member's four **ability slots**, in the hero panel's order (task 123; the third, `f`, was `c` until October 2026, when `c` became the ping; the fourth, the ultimate, moved from `r` to `g` in October 2026, when `r` became the reload, and then to **Space**, the remote trigger going to `g`). `q` and `e` are the **class actions**: an engineer **lays a mine** on the deck tile under the pointer / **throws a satchel charge** at it (hold `e` to aim, let go to throw; `g` sets them all off), and its `f` **lays a Healing Sentry** and its **Space**, the ultimate, **lays its sentry** (task 154); a soldier **throws a grenade** at it (hold `q` to see the reach and the burst's radius, let go to throw) / **charges a Stun Shot** at the pointer (hold `e` to aim, let go to charge), two seconds (walking as he likes) before it fires and bursts on the first enemy in its way, and its `f` is **Weak Spot** (passive, nothing to press) and its **Space** goes on a **Rampage**; a medic **drops a Heal Drone** / **beams the crew member under the pointer**, and unlinks when pressed on the one it holds or on nobody, its `f` is **Triage** (passive) and its **Space**, the ultimate, **switches its Healing Circle on or off** (task 153); a tank **raises its Riot Shield** or puts it down / **raises its Reflect Barrier**, its `f` is **Plated** (passive) and its **Space**, the ultimate, **throws the Bastion** over the crew round it (task 155) — see [Classes and levels](#classes-and-levels). The log says why not; nothing with a classless crew member |
| `c`, a middle click, or **Ctrl** + left click | **Ping** where the pointer is, on the deck or the map's chart: a mark in your colour on everybody's screen. A middle drag still pans |
| **Ctrl** + a slot's key, or **Ctrl-click** its box | **Ranks that ability up** rather than using it, for a skill point: the soldier's four abilities (task 124). It follows whatever key the slot is bound to |
| Middle-drag, or the pointer against the window's edge | Pan the view — the galaxy chart; over the deck the camera always follows the Bim you steer. The edge scroll's speed is on the Esc sheet's first page (0 turns it off). WASD panned and `v` let the camera go free until October 2026, when both went |
| `x` | **Attack**: the pointer turns into a red crosshair, and the next click on the deck plants an **attack banner** there. The crew nobody steers fight their way to it — taking the cover on the way, pushing on when nothing is in range — and hold it. `x` again, `Esc` or a right-click puts the crosshair away; the banner clicked where it already stands calls it off |
| `y` | **Retreat**: the crew nobody steers fall back to the ship and hold there. `y` again and they go back to keeping to your side. Nobody leaves a fight *aboard* the ship — cornered in your own hull they stand and shoot whatever they were told |
| `m` | The world map; during a mission it is only looked at |
| **Space** on the map between missions | **Propose** the place picked on the chart or the list; with nothing new picked, **Accept** the trip on the table. The button it presses says *(Space)*. It shares the key with the ultimate, which the map does not read there (October 2026) |
| Drag a box over one | Select it. Selecting shows its crew sheet |
| Click one | Select it — a click is just a box of no size. Only the Bim you steer takes your orders |
| Click empty floor / `Esc` | Deselect — the right-hand panels go with it. `Esc` first shuts whatever is up, innermost first: a cell's rows, a menu, a grid window |
| Drag a box / click, in a fight | Only your own Bim is picked: a bot cannot be selected while the crew are under arms |
| Right-click the floor | Send the Bim you steer there, whoever is selected — opens a door on the way if it must. The spot is pinged with green arrows closing on it (a cross where it cannot go) |
| Right-click an enemy | **Attack it**, the way Dota does: the pointer is a crosshair over an enemy you can see, and the click has the Bim you steer take its weapon out and shoot that one and nobody else — walking after it until it has a shot — until it is down or you give another order. Red brackets stand round the enemy for as long as the order does |
| **Shift** with a right-click, a right-drag or a menu row | The order **waits its turn** behind what the Bim is on and whatever was queued before it, the way RimWorld queues them (feature 69): the queued walks are drawn on the deck as a dashed thread with a pip at each spot, and the rest show on the agenda. A plain order afterwards calls the queue off. What cannot be begun when its turn comes — a bandage with no dressing left to wind — is dropped without a word; a walk with nowhere to go is refused at the click |
| Tray, bottom left | **Work** — which jobs come first; **Management** — what the crew keep doing of their own accord; **Inventory**, **Research** and **Skills**; on the ship, **Build** — lay parts out for the crew to build, where the shipyard is on |
| Point at anything | Top left says what it is |
| Point at a row that names a place | The place is ringed on the deck |
| `g` | The engineer's **remote trigger**: every satchel charge he has thrown goes off at once (task 154; it was **Space** until October 2026, when the ultimate took it). Nothing for any other class |
| **Esc** → **Pause** / **Resume**, **`** | Pause and set going again; set going. The world runs at 1× or not at all |
| Rest on an underlined word or a ? | It explains itself, after a third of a second |
| **Esc** → **Controls** | Every key, rebound by clicking it and pressing the new one. **My profile** is your own keys, kept on this computer (`~/.config/bims/keys`) and saved as you change them — the first time, a copy of the defaults; **Defaults** is the game's, shown but not changed there. **Reset to defaults** puts your profile back on them, and **Save as defaults** writes the keys shown into `keys.ron` where the game was started (or the file `BIMS_HOTKEYS` names): the keys a player with no profile of their own plays (October 2026) |

Nothing on the page explains itself in prose any more. The explanations are in
tooltips, and a tooltip is always *asked for* rather than stumbled into. Two
rules, and nothing else pops anything up:

- **An underlined word.** Where the thing already has a name — Health,
  "Speed" — the name carries it, dotted-underlined and with the help cursor.
  Hovering the row, the bar or the panel does nothing.
- **A "?".** Where there is no word of its own to underline, a small question
  mark sits beside the controls — beside a block of rows, say, that no one
  word belongs to.

Either opens after the pointer has rested on it for 300ms. The delay is the
point: crossing a panel on the way somewhere else sets nothing off. The ring a
row draws round the place it names is not one of these and needs no
affordance — nothing pops up, nothing is said, and it is gone the instant the
pointer moves on.

Either button opens a fixture's menu, and doing so leaves the selection alone;
a *sweep* across one is still a marquee. Right-clicking bare floor is still a
move order — the hit test decides which of the two a right-click meant. Nothing
is greyed out for being busy any more: a new errand takes over and the old one
goes on the agenda (below). Items are only ever greyed out for a reason of their
own — a locked door, a pack with no room left in it.

## Nothing is remote

Every menu item is an errand, including the ones that look like switches.
Asking to hold a door open, close it, lock it or unlock it sends the Bim
walking to that door's panel; the state changes at the moment its hand
arrives and not before. There is no way to reach into the room from outside
it.

That falls out of one shared pair of steps — walk to it, put a hand on it —
carrying which switch on the task rather than in the step, so a switch errand
queues, resumes and shows up on the agenda exactly like any other.

Where a door has two sides, the Bim goes to whichever panel it is nearest,
so it can let itself out as readily as in.

## Interrupting, and getting back to it

Send a selected Bim somewhere, or start it on something else, and the new thing
wins immediately. What it was doing is not thrown away: it goes on a queue with
its progress intact, and the Bim picks it up again as soon as it is free — after
walking to wherever it was sent, or after the errand that displaced it.

The queue is a stack. Each interruption goes on the *front*, so a chain always
resumes directly after whatever displaced it: interrupt a dressing with a walk
and the walk with a door to unlock, and the order back out is the door, the
walk, the dressing.

**Hold Shift and the new thing waits instead** (feature 69). A right-click on
the deck, a right-drag for a line, a row of a fixture's menu, a bandage on the
crew sheet, a gun picked up off the deck — given with Shift held, any of them
goes on the *back* of the same queue, behind what the Bim is on and whatever
was queued before it, the way RimWorld queues orders. The walks are drawn on
the deck as a dashed thread from where the Bim is bound now through every spot
in turn, a pip at each, until they are walked; the rest sit on the agenda under
their own names. When its turn comes an order is *begun* the way the row would
have begun it, against the room as it is then: a bandage queued behind a walk
finds the pack empty and is dropped without a word, the way the greyed row
would have refused it. A plain order — anything given without the key — calls
the queue off, though what the Bim had put down of its own is kept to be picked
up. Shift on a greyed row does nothing: what cannot be ordered now cannot be
queued either.

Resuming is the part with a trap in it. Standing steps assume the Bim is
already in the right place — a pair of hands on a wound works on whatever is in
front of them — so dropping the Bim back on the step it was interrupted at
would have it working on thin air in the middle of the floor. Instead, resuming
rewinds to the most recent *walking* step of that chain and lets the Bim walk
back first, then drops into the interrupted step with the time it had already
put in. The steps skipped on the way in are exactly the ones whose work the
room is already holding. What the room cannot hold is what was in the Bim's
hands, so that is snapshotted too, and put back after the walk.

### The agenda

The panel over the top-left of the deck is the checklist for all of this. One
row per chain — the one running first, then the queue in the order they come
back — each with a progress bar at the left-hand end, a tick box that is filled
for the chain actually running and empty for one that is waiting, and the name.
A queued row keeps the progress it had when it was put down, so you can see a
dressing sitting at 32% while the walk it was interrupted by runs at the top.
The panel hides itself when there is nothing on.

## The Management tab

> **Gone from the HUD** (feature 107), with the Work, Build, Research,
> View and Ship tabs beside it: the crew keep to what they were left on —
> autonomy on, gear not combined by itself, nothing kept made, every job
> at 3, nothing queued for research — and what follows is what the tab
> did while it was there.

**Management** in the tray is what the crew keep doing of their own
accord, and three things are on it: the **autonomy** box, whether the
crew take work up by themselves when they have nothing on; **Combine
matching gear**, whether they carry two of a kind at one tier to the
workbench and the better one back without being told (see *Armour*,
under *What the ship will make*); and the **crafts**, a row for each
thing a bench aboard can make — how many there are, where they are kept,
and how many to *keep made* — which is how the drug lab is asked for
medkits (*Making things*). The food, the stew and the dressings each crew
member kept in its pack were rows here too, until the needs went and the
medicine became everybody's charges.

## Work priorities

The **Work** tab in the tray is the answer to "what should they do first".
One row per job, a number from **1 to 5** in a box beside each, and **1 is
done first**. Click a box and it steps one less important, from 5 back round
to 1. Everything starts at **3**, all equal, so out of the box this changes
nothing at all, which is deliberate: a default that is already an opinion is
a default you have to undo before you can use anything.

| | |
| --- | --- |
| **Hauling** | carrying gear between the lockers and the workbench |
| **Making things** | working a bench the crafts table has an order for |
| **Building** | putting a laid-out part together, once the crew can pay for it |
| **Medical** | reviving a downed crewmate (task 120) |

The colour of the box says what the number means without anybody having to
remember which end is which: warm at the top of the list, cold at the bottom,
five steps across the palette. At the top of the panel are three ways to
reorder the rows — **Priority 1→5**, **Priority 5→1** and **Name A–Z**.

Sorting is something you *press*, not a rule that stays on. If the list
re-sorted itself live, the row you just clicked would jump out from under the
pointer — and at the wrap from 5 back to 1 it would jump the whole length of
the list. So the button reorders the rows there and then, and the mark showing
which order was applied is cleared the moment a box is clicked, because the
list may no longer be in it.

Resting on a row rings the places the job is done at — **every one of that
kind**, so a ship with two benches sees both rung. Building and medical ring
nothing: a site is wherever it was laid out and a patient wherever it fell.
Nothing pops up and nothing is said: a highlight is not a tooltip (see *What
the pointer is over*).

**Medical is the one row that can interrupt.** It is on offer to a bot
while a crewmate is downed — the nearest one it can walk to, out of harm
or with the fight quiet, and nobody else already reviving it — and at any
number from 2 to 5 it waits its turn like the rest. Set it to **1** and
it is urgent: a bot drops whatever it is on the moment a crewmate is
down, the errand going onto the queue the way any interruption would
push it. Set it to **never** and nobody revives of their own accord;
your own order on a downed body still works.

### What it actually changes

Only what a Bim takes on **of its own accord**. A Bim with nothing on looks
at whatever work is going — a wound to dress, a site to build, a gun to
carry to the bench — and does the one nearest the top of the list.

**Hauling is gear, and nothing else.** A gun or a piece of armour carried
from the lockers to the workbench, and the upgraded one carried back (see
*Armour* under *What the ship will make*), is offered and taken at
hauling's own number. It was a harvest carried to the cold store as well
while there was a bay to harvest, and materials to a construction site
until the money rework: there are none to carry.

## What the pointer is over

Top left, above the agenda, one line says **what is under the pointer**:
deck plating, a bulkhead, a door, a bench, the lockers, a bunk, the cold
store, the table — and *outside the hull* if the pointer is off the ship
altogether. Where there is something worth saying about the state of it,
it says that too: a door **locked**, **held open**, **open** or **shut**.
It does not name what is lying on a tile: the blood on the deck is drawn
and not named, so a stained tile reads as the deck it lies on.

The readout is a different question from a click, and answers accordingly. A
click asks *what would this act on*, which is why only the few things with a
menu behind them are hit-testable and why each of those is given a few pixels of
slack. The readout asks *what is this*, so everything aboard has a name and
nothing is expanded: a pixel beside a bench is deck, not bench.

**A highlight is not a tooltip.** Resting on a row of a panel that names a
place — a Work row, a fixture's — rings that place on the deck, in the
ship's own cyan rather than the green that means "selected" or the warm
colours that mean trouble. It does not go through the tooltip machinery:
nothing pops up, nothing is said, and a pointer crossing the panel on its
way somewhere else lights a fixture for a moment and leaves nothing behind.
The ring is worked out afresh every frame from what is hovered, so a panel
folding away under the pointer cannot leave one lit.

## Recruiting

Press **r** and the Bim is under direct orders. It stops doing anything of its
own accord: no work taken up, nothing picked back up off the queue, and none
of the pottering about it does between jobs. It stands where it is put until
told otherwise. Press **r** again to let it go.

Everything the player asks still works: move orders, menus, all of it.
Whatever it was in the middle of when recruited is left to finish rather than
cancelled, since throwing away a half-dressed wound for tidiness would be
worse, and a right click interrupts it anyway. The queue is kept rather than
emptied, so letting the Bim go picks up where it left off.

It shows in two places, because one of them is easy to miss: a badge in the
header, and a broken amber ring around the Bim itself, in a colour used for
nothing else and drawn whether or not the Bim happens to be selected.

### Combat mode, and the inventory

> **Since task 113 there is no inventory**: no pack, no looting, and a
> body knocked down keeps its gun in its hand. What a Bim carries is its
> loadout and its charges ([the Armory](#the-armory)).

**The rest of the crew take arms on their own.** Whenever an enemy is
within thirty tiles of anybody or in anybody's sight — a compartment or
two away, not merely somewhere on the station you are docked at — or a
crew member has been hit in the last half minute, the
header says **To arms — an enemy is near** and every crew member but the
one you steer is in combat mode of their own accord: they take a weapon
out of their pack if their hand is empty, draw it, and **gather round
you** — a slot each beside and behind the Bim you steer, which they
keep to as you move, shooting whatever they can see from there. Only
when one of them **sees an enemy for itself** does it go and fight for
itself — walking to cover within range, peeking round it — so a squad
does not charge in headfirst after something only the log knows about.
**A click never orders them**: a right-click moves the Bim you steer and
nobody else, and while the crew are under arms a bot cannot be selected
at all, so a click in a fight is aimed at the fight. What moves them is
your two orders (**X**, the attack banner, and **Y**, back to the ship)
and **a commander's squad orders**: they command the crew nobody is
steering — attack, fall back, stand ground — and never move, hold or aim
a Bim a player steers. **A right-click on an enemy** has the Bim you
steer keep at that one alone until it is down. The alarm lasts until
nobody is near, nobody has seen an enemy and nobody has been hit for half
a minute, when they go back to their work, however many machines are
still standing somewhere on the station. (A crew member downed in the
fight stays down until somebody revives it, or dies when its thirty
seconds run out; see *Getting hurt*.) A hired mercenary does the
same. The Bim you steer is yours alone: recruit it yourself or leave it
to its work; the alarm never touches it.

A recruited Bim is in **combat mode**: it draws its weapon — held in both
hands, out in front — and, whenever an enemy is in range and in its own
line of sight — the peek round a wall included — it fires. Everybody
carries a **hand laser pistol** from the start; the other four guns are
bought, and a Bim swaps to one out of its pack. It does nothing else about
the enemy: it stands where it was put, as a recruited Bim does, and shoots
from there; walking it somewhere is still yours to order, and it holds its
fire while it walks. Let it go and the weapon is holstered. Your own
Bim is heard drawing and holstering — the same recording, the holstering
slowed a little — and nobody else is.

Every shot is a **bolt** that flies — at the weapon's speed, until it
reaches a body, a wall or the end of its range — and is always drawn,
whatever it flies through, each weapon's own: the pistol's dash, the
shotgun's fan of orange pellets, the rifle's short yellow tracer, the
sniper's long white-blue streak. Friendly fire is **blue**, an enemy's
**red**. A weapon's numbers are two points on a line: an **accuracy** and
a **damage** that hold out to its sweet distance and fall off in a
straight line to what they are at its range. A shot's odds of landing are
the accuracy at the distance to the target, rolled when it is fired; the
damage is read where the bolt *lands*, at the distance it flew, and comes
off the body it lands on; a miss flies visibly wide. The **pistol**
reaches twelve tiles, 95% falling to 65%, six a hit, one and a half a
second. The **shotgun** reaches ten: 90% and fifty a hit out to four
tiles, 60% and thirty at ten, one shot every four seconds. The **auto
rifle** reaches sixteen, 85% to eight tiles and half that at the end, five
a hit — but a pull of the trigger is a **burst** of eight in two seconds,
then two seconds to recharge, and every shot of the burst is rolled and
aimed afresh. The **sniper rifle** reaches thirty-five: a certain hit and
forty-five out to twenty tiles, 70% and twenty-five at the end, a shot every four
seconds, and its bolt is the fastest thing in the game. The **schword** is
no gun: a blade with a laser edge, thirty-five a swing every two seconds —
the swing takes half a second and the blow lands as it ends, so a body
that steps back in time is missed — on whoever is within arm's reach —
a tile and a bit — and a cut is a
**three-unit wound** that bleeds three times what a shot does and throws
blood over the tiles round the body. **Every enemy is a machine**, ringed
in red, with no blood to bleed and nothing to loot, and what each of the
three does is *The fight*, under *What the ship will make*: a Trooper's
forearm is a gun, whose shots fly in the crew's room in red and land on
the crew's own bodies; a Husk's claws are a blow at arm's length; and a
Warden's Unmaker takes armour off rather than blood. A hit on a crew
member is on the head, the body or the legs, and what it does is
[Getting hurt](#getting-hurt).

**Every gun has a magazine** (October 2026): so many shots, then a
reload, and the magazine is full again — for ever, since there is no
ammunition to run out of, only the seconds a reload takes. The **pistol**
holds twelve and reloads in 1.2 s, the **auto rifle** thirty in 1.8 s,
the **shotgun** six shells put in one by one over 3.5 s, the **sniper
rifle** four in 2.4 s, and the **minigun** a hundred at ten a second,
then 4 s (its twenty-bolt burst and five-second cool until later in
October, when it went to three times the price at a trader); the rail
lance's one slug in five seconds is its own reload and has none, and
nor has a blade. The guns were paid for it: the pistol and the auto rifle hit
**2 harder** (8 a shot, and 7 near and 6.4 far), the shotgun fires
**twice as often** (a pull every two seconds), the sniper nothing. The
last shot of a magazine begins the reload; **R** (the *reload* key)
reloads the Bim you steer sooner, shots left or not; a bot with nothing
to shoot at tops its own up; and a gun put in the hand comes full.
Everybody reloads — the crew, a station's people, a Trooper's arm, and
the engineer's sentry with its minigun. The hero panel shows the shots left over what the
magazine holds (warm at a quarter, red empty) and, while it reloads, a
bar and *Reloading*; a gun's tooltip says its magazine. A reload is heard
— the gun's recording, the shotgun's shells — your own over the rest.
What a gun does a second, to the bots' tactics and in the tooltips, is
over a magazine and its reload.

**No weapon loses damage over distance** (October 2026): a hit does the
same out to the end of the range — the shotgun sixty, the sniper fifty-four,
the auto rifle seven — and only the odds of landing it fall off. What the
numbers above say about less damage at range is the history.

Two things a corridor fight turns on. **Peeking exposes the peek.** A Bim
that aims from the peek beside a wall leans out to it, and that is where
the enemy shoots at — but it is in cover, and a bolt reaching a body that
is peeking is **dodged half the time** and flies on. A line of
**sandbags** is **low cover**: a body can walk over it and see over it,
but standing close behind it — the tile next to it, on the far side from
the shooter — half the bolts coming over it miss, the same as a peek; a
bot picks such a spot to shoot from, and every station's corridors have a
barricade of them; a ship can build them too, under Structure. **Walking
halves your aim.** Every gun shoots on the move at half its odds, so a
bot with a shot from where it stands stays put and takes it, and walks
only for cover or when it has no shot at all; a Bim you send across a
room fires as it goes, at half. And **a schword is a melee**: a Bim with
one is **locked** with anything within arm's reach and swings, and
walking out of reach ends it. The log says *locked in melee* the step it
happens. A crew member dying, treated or dead is said in the log, and so
is every hit.

Tab opens a Bim's **inventory** in a window of its own (a recruit does
not — a fight is not the moment for a window over the deck), and the tray's
**Inventory** tab shows the same for whoever is selected, recruited or not:
three armour slots down the left — head, body, legs — each with the icon
of the piece worn there, a bar of the health it has left and its numbers
(`+15 hp · 2 prot`); the weapon slot on the right with its icon and its
numbers beside it, read off the two-point curve: **Range**, **Accuracy**
("90% to 4 tiles, 60% at 10"; the pistol, with no sweet distance, reads
"95% up close, 65% at 12 tiles"), **Damage** ("100 to 4 tiles, 60 at
10", or "6 a shot"), **Shot speed**, **Fire rate** ("1.5 a second") or
**Burst** ("8 in 2 s, then 2 s") and **DPS**, which is the burst times
the fire rate times the damage — what it could do a second with every
shot landing; a schword reads "Melee — 70 a swing every 2 s", its
**Reach** and its DPS instead. While the Bim is locked in a melee the
header says so — *combat mode — locked in melee*, with a `?` that
explains the fists — and *locked in melee* sits over its name on the
deck; and under the weapon the **pack** on the Bim's back, a grid ten
cells across by five down with every thing in it over its footprint.
Beside each armour slot is the count of open wounds on that part of the
body and a **Bandage** button — see the next section — and under the
lot, how many dressings the Bim you steer has in its own pack. A box of
dressings is two cells by two and holds five, with the count in its
corner; right-click it for **Bandage all wounds**.

**Inventories are grids**, and every container aboard is looked at the
same way. Click the **armoury** (or the drug lab — either opens the
lockers) or a **shelf**, or pick the **cold store** on the Nearby strip,
and a window opens showing what the hold keeps in that class, laid out
as its grid: every thing over its footprint, a piece of armour with a
sliver of its health under it, and a stack with its count in the corner.
The window is the class, not the cupboard: a second shelf is the same
grid, and a helm put away across a shelf turns up in the lockers' window,
because the lockers are where the hold counts it. Clicking a container
also opens the inventory of the Bim shown beside it and walks that Bim
over, since nothing can be moved until it stands within **two tiles**.
**Right-click** a cell for the rows —
**Take** from a container into the pack, **Store** from the pack into a
container, **Equip** what is in a pack cell (whatever was worn comes off
into that cell; a weapon swaps with the one in hand), **Discard** — and
right-click a worn slot to **Unequip** it into the pack. **Ctrl-click** is
the quick move: a cell in a container goes straight into the pack, a cell
in the pack straight into the open container. A row that cannot go says
why in grey — *walk over first*, *the pack is full*, *no room left in the
lockers* — and a ctrl-click that cannot go opens the rows instead of
doing nothing. Every move is a command, so on every player's ship the
hold agrees. Hover a cell and it names the thing and its numbers; what a
worn piece does for the body wearing it is the next section.

**A crewmate that is down can be looted** — dead or out cold; nobody on
their feet can be, and a machine carries nothing to take. Right-click the
body and the row is **Loot** (beside the bandage rows for a crewmate who
is only out cold). It walks your Bim over and opens the **Loot** window:
the body's pack, and under it a row of four — head, body, legs, and the
weapon in its hand — drawn like any container's grid, a worn piece with
the sliver of health it has left. **Take** on a right-click, or
ctrl-click, moves one thing into your Bim's pack, once it stands within
two tiles and has room; a piece comes off the body as it is, broken or
not, a weapon goes into the pack to be equipped from there, and the
stripped body draws bare. Nothing can be put onto a body. Down and reach
are checked when the command lands, not when the window opened: a
crewmate who comes round meanwhile is no longer a body — the window shuts
on them — and the command is refused *not down*.

## Getting hurt

A Bim's health is **one bar of a hundred hit points** (task 120) — every
Bim alike: the crew, a station's people, a town's, the Manufacturers.
There is no blood to lose, no wound to dress, no trauma, no lasting
injury and no leg lost, and nothing mends by itself.

A shot still lands on a part — one in twenty the head, three in four the
body, one in five the legs — but the part only says **which piece of
armour takes it first**; what the armour lets through comes off the bar.
A blow in a melee lands the same way. A hit that takes hit points throws
a little blood on the deck; one the armour takes whole throws none.
Under **twenty** a Bim is badly hurt and drips as it walks, leaving a
trail.

**At nought a Bim is downed.** It falls where it stands, cannot act and
is no target for anybody, and a **thirty-second countdown** starts — a
red ring over the body on the deck, emptying, with the seconds in the
middle. The countdown runs with the world and stands still while it is
paused. When it runs out the Bim is dead.

**Any crew Bim revives a downed crewmate** by going over and staying
beside it: **ten seconds**, a medic **four**, *Trauma Kit* two off either
(never under one). The reviver stands still and fires nothing while it
works; being shot does not stop it, and one reviver is all that counts.
Holding `g` beside a downed crewmate has your own Bim do it, and a
right-click on one opens a menu of two rows, *Get up* and *Carry*, each
greyed with the reason when it cannot be done; a bot already on its way
gives way to you. A green bar over the body, above the countdown ring,
fills as the reviver's hands-on seconds run — and the
**bots revive** their downed crewmates of their own accord, when the
body is out of harm or the fight is quiet, and **the moment a wave is
cleared** — the last enemy down, nothing left to shoot — they go at
once, shot at a moment before or not, to the **downed players first**
and only then to the downed bots, the nearest of each first — **a medic
bot first** (the
class, or a hired field medic): the other bots leave a body to a medic
that is free to go to it, and take it themselves only when there is no
medic, the medic is down, busy with another body or in a fight of its
own. Nobody runs from a fight on the way to nought: a Bim shot down is
downed where it stands. A revived Bim is up at
**thirty** hit points and walks **30 % slower** for the rest of the
mission (a second down does not slow it more). A Manufacturer is never
revived, and a station's or a town's people do not revive each other. A
downed Bim can be carried by a medic, as a knocked-out one could.

A crew member's bar is **whole again at the start of every mission**,
and the slow is gone at its end. Nothing else heals: the medic's beam
does (see *The medic*), and a few relics.

There are **no bandages and no medkits** any more — no charges, no
cooldowns, nothing sold — and the drug lab is gone from the part list.

## Time

`crates/game/src/clock.rs` keeps the room's clock: minutes since midnight,
and which day it is, on the `time` crate's lengths. One real second is one
game minute at 1×, so at 24× a minute goes by in two and a half frames. In
a run the room's clock stands still with the world's — only travel moves
the day (see [the loop](#the-loop-world-map-travel-missions)) — and
everything inside a mission is timed on the world's **mission clock**
instead, which counts the steps since the crew arrived.

Nothing about time comes out of the room as a string: the app reads the
minute count and formats it. The light level comes from the same clock,
eased in at dawn and out at dusk, and is laid over the finished frame as
one tinted rectangle — so nothing in `room.rs` has to know what time it is.

## The look of it

Everything is drawn from the two primitives in `crates/game/src/draw.rs`, so "futuristic"
here is a matter of palette and of what gets a light on it rather than of any
new drawing machinery. The deck is dark blue-grey, the fittings are composite
panel in three shades, and one cyan running light is picked up by every powered
surface: the counter fascia, the coils etched into the hob, the rim of the
table, the jambs of a door. Warm colours are held back for trouble, which is
why a locked door reads instantly.

The first two of any crew are told apart by their hair and the yoke on their
coverall, not by where they happen to be standing: James has cropped hair and
a pale yoke, Kate a mauve one with hers worn long, which from directly above is
a second ellipse behind the head; everybody after them is dealt a build and a
look of their own (*The multiplayer seam*). The coverall is the ship's blue on
the crew and the station's orange on the station's people, so a joined deck
says at a glance who is going ashore when the crew leave. Their names are
written over them.

The page around it is arranged the same way: nothing is framed. The deck runs to
the edge of the window, the canvas carries no border of its own, and every panel
sits flush in a corner of it — the portraits and the top frame along the
top, a picked crewmate's health and crew sheet on the right, the hero panel
at the foot, the tray in the bottom-left corner ([The HUD](#the-hud)). A
rounded box drawn round the whole interface reads as a window frame inside
a window, which is one frame too many.

The tray is the exception to everything being small. It is set a size larger
than the rest — wider, and bigger type — because what is in it, the work
list and the two trees, is read and edited rather than glanced at.

## How it fits together

The simulation is entirely Rust, and so is the window. Each frame the app
steps whichever screen is up and asks it for a flat buffer of shapes; the
buffer is tessellated into triangles (`crates/app/src/shapes.rs`) and drawn
as Bevy meshes under egui's panels (feature 97), and the words — names over
heads, labels on the map, the readouts — go on after. The rules crates hand
over numbers and shapes and nothing else: no string leaves them, and no
sound either — the room says a door slid or a shot was fired as a *cue*,
and the app owns the recording — so the native server that will one day
run the same crates has nothing to say and nothing to disagree about.
`cargo build` is the whole pipeline.

### Thirteen crates

The repository is a cargo workspace and everything is under `crates/`. The
split is not tidiness — each line of it is something that has to give the same
answer in two places at once:

| Crate | What it is | Who else needs it |
| --- | --- | --- |
| `app` | The window: Bevy and egui, the screens, the pointer, the words. The game's one binary | — |
| `game` | The room: the simulation, and the shapes it draws itself as | `world`, which runs it aboard the ship and on every station's deck |
| `ship` | The design phase and the game it starts: `Session`, the editor, the two cameras, the painters | `app` |
| `lobby` | The World tab's galaxy: a camera over it, a pick, and the system diagram | `app` |
| `world` | One star system, the ship in it, the run, and the clocks they all run on | `ship`; the native server, which has to run the identical loop |
| `flight` | What a design does when you push it, and the closed-form plan the flown trip used | `world`, which quotes a run's trips off the ship's accelerations; `ship` and `app`, which read them |
| `shipdesign` | What a ship is made of and the rules for putting one together | `ship`, `flight` and `world`; the native server, which has to admit the same ships |
| `worldgen` | The galaxy, what is in each system, and station blueprints | The native server that will one day be authoritative, which has to generate the identical world from the same seed |
| `physics` | Ship mass, engine thrust, travel time. Pure arithmetic | `worldgen`, to check its layouts; `shipdesign` and `flight`, to weigh a real ship; `world`, to say how long a trip is |
| `economy` | Money: whole euros, the shared pool, what a station charges and which hold it goes in | `shipdesign`, `world` and `ship`; anything that ever charges for anything later |
| `time` | How long a minute, an hour and a day are | All of them — a day that is two lengths is two games |
| `wire` | What a client and the relay say to each other: rooms, codes, and bytes passed along | `app` and `server`, and nothing else |
| `server` | The relay, `bims-server`: it introduces the players and passes their bytes between them | Nobody — it links `wire` and nothing else of the workspace |

The `health` crate that was a fourteenth — a body's radiation dose, its
sickness and its cancer — went with the radiation (feature 104); what a
body is made of, its parts, its blood and its wounds, was always the
room's (`crates/game/src/health.rs`).

`cargo test --workspace` runs every crate's tests, and `nix flake check`
builds the app and runs them. `game` has tests of its own on a bare room
(`Game::bare`), with three native probes in `scratchpad/` beside them;
`./check smoke`
opens four windows for sixty frames each and keeps a picture of three of
them.

### Inside the room

Relative to `crates/game/src/`:

| File | What lives there |
| --- | --- |
| `lib.rs` | The module list, and why `time` is reached as `crate::time` |
| `game.rs` | Ties the room, the Bims and their running tasks together; input |
| `room.rs` | The room: its layout, its fixtures as solids, and how it is drawn |
| `health.rs` | The body's three parts, the blood, wounds, traumas and bandages |
| `blood.rs` | The blood on the deck: where it lies, and the boots that carry it |
| `fixtures.rs` | The fixtures the room only draws — the galley, the table and chairs, the bunks, the broom locker, the bays and fields, the heads — as a fresh room draws them, for the deck and the designer alike |
| `sight.rs` | What each Bim can see, traced on the tile grid, and the peek round a wall |
| `combat.rs` | Weapons, bolts, hits and shots; the enemy's choice of where to stand |
| `droid.rs` | The machines: their bodies, and how they fight |
| `routine.rs` | A station's people and the rounds their roles walk |
| `task.rs` | The scripted chains — a dressing, a treatment, a part put together, a gun carried to the bench |
| `clock.rs` | The time of day, and how much light there is — restated from the `time` crate as `f32`, not defined again |
| `character.rs` | The Bim — how it decides where to go, and how it is drawn |
| `draw.rs` | The shape buffer and the local frame used for sprites |
| `math.rs`, `rng.rs` | Vectors, rectangles, angles, and a PCG32 generator |

And in `crates/app/src/`:

| File | What lives there |
| --- | --- |
| `main.rs` | Every command, and the screen state machine |
| `screens/builder.rs` | The menu, the setup screen and the lobby, with the World tab |
| `screens/designer.rs` | The design phase, and the `Net` seam every edit goes through |
| `screens/game.rs` | The game: the deck, the map, the station |
| `screens/hud.rs` | The HUD (feature 107): the portraits, the top frame and its warning, the hero panel, the log |
| `screens/worldmap.rs` | The world map: where a trip can go, the vote on one, *Back to ship* |
| `crew.rs` | The crew's panels: the side panel, the character sheet, the tray, the menus on a door or a body |
| `names.rs` | Every word on the screen, indexed by the codes the crates hand over |
| `shapes.rs` | The shape buffer, turned into triangles |
| `sound.rs` | Every sound, the way `names.rs` is every word: the room's cues and the world's events played as clips cut from `Sounds/` |
| `settings.rs` | The Esc sheet: the UI scale, the audio volumes, the keys, and the run put back to where it opened |
| `canvas.rs`, `theme.rs`, `format.rs`, `dev.rs` | The pointer, the palette and widgets, numbers as words, and the smoke run |

### Getting about

Every walk — a right-click order, and every leg of a scripted job — is planned
on a grid in `crates/game/src/nav.rs`. Obstacles are inflated by the body radius before the
search, so a route that exists on the grid is one the Bim can physically walk
without clipping a corner. A* returns a staircase of cells, which is then pulled
straight by dropping every waypoint that can be skipped with a clear line of
sight; what is left is the two or three straight legs a person would actually
walk. A destination that is not standable — inside the table, behind the counter
— snaps to the nearest spot that is, so an order there means "the floor beside
it" rather than nothing at all.

The steering and the collision push-out are still there, but only for the
wander and as a backstop. While following a path the Bim trusts the plan.

### Ordering the Bim through a door

A ship's doors and a station's are powered: a body walking up to one opens it,
and it shuts a moment after the doorway is clear. So an unlocked door is never
in the way of an order. The pathfinder plans straight through it, open or shut,
since it will be open by the time the Bim gets there, and the Bim does not stop
at a panel on the way. There is one grid for the deck, and nothing is worked
out twice.

A **locked** door is the one state of a door the pathfinder knows about: it is
a wall, and the grid is built again the moment a lock changes. An order to
somewhere only a locked door leads to has no route, so it is refused — the Bim
does not walk over and fail, and it does not drop what it was doing either. A
lock ordered while somebody is in the doorway waits for them, since the leaves
never close on a body, and a walk already under way through a door locked since
it was planned is caught by the watchdog that catches a frozen walk: it is
planned again from where the Bim stands, or put down if there is no way round.

What a door does is changed by hand, from the menu on it: hold it open, let it
close again, lock it, unlock it. Each is an errand of its own — the Bim walks
to the door's panel, a pace out of the opening on its own side, and works it —
and it goes on the agenda like any other.

Two things can come back from an order instead of a walk:

- **Nowhere to go.** No route there as the doors stand: a locked door in the
  way, or a spot no body can reach.
- **Ignored**, if the Bim is not one this player may order, is dead, or is
  outside the hull.

The refusal is said out loud, because an order that quietly does nothing reads
as a broken click: the log says it could not be done, there being no way there
at all, and a cross is drawn where the order landed.

### A chain never starts somewhere it cannot walk to

An empty route reports itself *arrived* the instant it is handed over — the
alternative, waiting on a walk that will never finish, hangs the chain for good.
That is fine for one step and quietly disastrous for a whole chain: every
remaining step finds itself already arrived and runs in the same frame, until
one of them sets a position outright and flings the Bim across the deck. A Bim
shut in behind a door and started on an errand on the far side of it once
turned up there without having walked a step of the way.

Two things stop it now. A walk with nowhere to go gives the chain up rather than
taking it for an arrival, putting the world back in a state the Bim can be left
in. And no chain is begun at all unless the Bim can reach the place it starts
at, so a Bim behind a shut door does not set off for the bench on the other
side of it. Queued chains are held back the same way rather than dropped —
they wait on the queue for the door.

Waiting is only right when the Bim can do nothing about it. A shut door is not
that: the panel is on the inside too, so a Bim that finds its work out of reach
behind one goes and opens it, and the work comes round again with the way
clear. Only a *locked* door is a real wait.

Reachability is worked out afresh every frame rather than cached, so opening or
unlocking a door needs no nudge: the moment the way is clear the Bim takes up
whatever it could not reach a moment ago, queued work included. Queued work it
cannot reach does not count as having something on, either, or a Bim would
stand over an unreachable errand with everything else it could be doing held
up behind it.

### Simulation speed

The speed control multiplies how many fixed 1/60 steps run per frame, never the
size of a step. At 24x a single stretched step would move the Bim four times
its own body in one frame and it would pass straight through the counter;
twenty-four normal steps give exactly the result 1x would, just sooner.
`MAX_STEPS_PER_FRAME` caps the catch-up so a backgrounded tab cannot come back
and lock the page up. (Since task 119 the game itself runs at 1× or not at
all; the room's own slider is a test room's.)

### The wander

A plain per-frame random direction looks like vibration, not walking. Instead
the Bim commits to a leg of a journey at a time: a direction and a speed held
for a second or three, then either a pause or a fresh leg. Course changes are
usually gentle and occasionally a complete change of mind, and the body eases
towards each new heading rather than snapping to it.

Near the walls and the furniture it steers rather than bounces, and the steering
bends the Bim's *intent* as well as its current heading — otherwise it turns
back into the wall the moment it comes clear, and spends its life hugging the
edge. A push-out pass after each move is the backstop that keeps it out of the
table when steering is not enough.

### The draw buffer

Twelve floats per shape: `kind, x, y, w, h, rot, radius, line, r, g, b, a`.
`kind` is 0 for a rectangle and 1 for an ellipse; everything on screen is built
from those two. Rotation is about the shape's own centre, and a non-zero `line`
strokes the outline instead of filling it; 2 is a right-angled triangle, the
bottom-left half of its box, which the diagonal hull plates are. `draw::STRIDE`
is the stride, and `crates/app/src/shapes.rs` asserts its own copy equals it.

If you change the layout, note that the replay in `shapes.rs` assumes the
field *order* in `crates/game/src/draw.rs`.

One trap worth knowing: `f32::clamp` panics when its bounds are crossed. The
room uses the branchless `clamp` in `crates/game/src/math.rs` instead, which
dates from when the panic path cost 19 KB of a wasm binary; it is still the
one to use, because a panic in a step is a stall with no message.
