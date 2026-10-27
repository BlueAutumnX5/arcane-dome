# Changes

## Next version (in progress)

- New weapon: the **Hail Rifle** (Automatic). It sprays balls of ice that travel at running pace, drop a little over distance, and bounce off walls, floors and ceilings until they hit someone (or melt after 8 seconds). Every ball hits for 4.5, even after bouncing. 100 shots a clip. It takes the Assault Rifle's place for now; the Assault Rifle returns when weapons can be bought
- New HUD on the PC:
  - Health sits at the top centre ("Health 97%", your name, "Shield 69%"). The bar fades from green through yellow to red as you lose health, with a thin yellow shield strip under it (phones get the same colours in their own spot)
  - The weapon in your hands is pictured top left, with its clip below it (or reload progress)
  - The timer and match name are top right, with the kill feed under them; the FPS counter moved to the bottom left
  - The weapon bar shows each weapon you hold with its name above its picture and an ammo strip along the top; the one in hand stands taller; one you hold but have run dry is greyed out; empty slots are low stubs
  - Every weapon has its own picture, drawn by the game (each weapon file picks one with `icon`)
- New match type: **King of the Hill**. Score a point for every second your side alone stands in the hill (the zone in the spotlight); nobody scores while it is contested. The hill moves to another zone every minute, or as often as you pick in Quick Match (or never). Free-for-all or teams
- New match type: **Domination** (teams). Stand in a zone to capture it: 6 seconds alone, faster with teammates, and an enemy inside stops it. Every zone your team holds earns points; first to 200
- New match type: **Territories** (2 teams). The zones form a line from one base to the other; you can only capture the zone next to one you hold. Hold every zone to score, and the line resets; first to 3
- New match type: **Capture the Flag** (2 to 4 teams). Grab an enemy flag and bring it to your own while yours is home. A dropped flag goes home after 15 seconds, or at once when its team touches it; first to 3 captures
- Zones show who holds them and how far a capture has got; flags fly from their bases and their carriers' backs; the top of the screen shows the hill's holder, every zone's owner, or where your flag is; arrows at the screen edge point to the hill, your flag, or home when you carry one
- Bots play every objective: they hold the hill, capture and guard zones, push the line, take flags home, chase flag carriers and return dropped flags
- All three maps have zones and flag bases for every team count they offer
- Map editor: Zone and Flag tools (with their size and team in the panel); the top bar says when a match type the map offers is missing its zones or flags; Map Settings lists match types two to a row
- Match types are offered in the order they are listed, Deathmatch first; team-only ones skip the free-for-all choice
- New match type: **Juggernaut**. One random fighter is the Juggernaut: red, in a beam of light, with 500 health and 200 shield; everyone else is on the blue side, hunting them. Kill the Juggernaut to become it. If the Juggernaut dies to the map or their own blast, someone at random takes over. An arrow at the screen edge points to the Juggernaut when they are off screen
  - Quick Match lets you choose what scores: kills as the Juggernaut, killing the Juggernaut, or both (one always stays on), plus a bonus that makes kills as the Juggernaut worth 2. The score limit is yours to set (10 to start)
- New match type: **Gun Game**, free-for-all or in teams (Team Gun Game, where a kill by anyone on the team moves the whole team up). Every kill moves you to the next weapon, from easy to awkward: Hail Rifle, Leech SMG, Laser, Shotgun, Pistol, Flamethrower, Marksman Rifle, Rocket Launcher, Sniper Rifle, Grenade Launcher, then the Shock Baton. A kill with the Baton wins
  - Everyone carries the Shock Baton beside their current weapon (switch with Q, E or the mouse wheel); it only exists in Gun Game
  - Quick Match switches: a melee kill sends the victim back one weapon, and so does dying by your own fire or to the map
  - Ammo never runs out in Gun Game, and weapon pickups, ammo boxes and weapon stations are left out
- Easy bots see a little wider, notice you sooner, turn faster, and stand their ground more instead of strafing and running off
- The FPS counter (Settings) now also shows the game's memory use, RAM and (on Linux) graphics memory, to help track down memory problems
- Bahasa Indonesia: pick it under Settings > Language (the game starts in it if your computer or phone is set to Indonesian). Menus, the match, weapons, maps and modes are all translated; the map editor and this page stay in English for now
- Android version (Android 9 and newer), with touch controls:
  - Left stick walks, and down crouches or drops; a Jump button (tap again for the double jump)
  - Fire button: hold it to aim at the nearest enemy you can see (red brackets mark it), or drag away from it to aim yourself
  - Reload and Switch buttons, a tap-to-pick weapon bar, and Pause and Scoreboard buttons; the phone's Back button pauses and goes back
  - Menus and the HUD are bigger on a phone; long pages scroll by dragging; buttons press when you lift your finger, so dragging never presses one
  - Menu slides are off on Android for now
  - The match pauses itself when you leave the app
  - The map editor stays on the PC version for now
- Map editor is complete:
  - Validate (V): checks that bots can reach every spawn, pickup and gadget, and that no spawn stands in lava or water; click a problem to fly the camera to it
  - Test Play (T): play the map against bots straight from the editor, then come back with nothing lost
  - Publish (Ctrl+P): put a map that passes Validate into Quick Match (never writes a map the game could not load)
  - Export (Ctrl+E) and Import (Ctrl+I): share maps through the Arcane Dome Maps folder in Documents, or drop a map file onto the editor
  - Map Settings (M): name, description, size, team counts and match types
- Long text in windows (like file paths) now wraps instead of running off the edge
- Fixed: the version history page could not be scrolled (it kept jumping back to the top)
- Bullets fly on past their range: they slow in the air, drop a little, and hit softer the slower they are
- Bullets leave marks where they hit walls and floors (the last 512 stay); flamethrower flames bounce
- Each weapon's file can set what its shots do on walls (mark, bounce or vanish), plus their drag and drop
- The camera looks further ahead with long-range weapons (sniper and marksman furthest)
- Bots aim more like people: they often misjudge their first lob, watch where it lands, and walk their shots in (sometimes the first one is spot on); their first swing onto a new enemy overshoots a little; being hit shakes their aim for a moment

## v0.1.2

- Map editor: draw terrain shapes, place spawns, pickups and gadgets, undo and redo, save drafts, open any map or start a new one
- Every editor action has a button with its shortcut, and every button explains itself when you rest the mouse on it
- Snap Grid can be switched on and off
- Instructions page inside the editor
- Asks before leaving with unsaved changes
- Menu pages slide into each other
- New main menu: Quick Match (the old Match Setup), Settings, View Changes, and a first look at what is coming
- Settings: fullscreen, menu slides, blood effects, FPS counter (saved between runs)
- F11 switches fullscreen on and off
- Nameplates: the bar over each fighter is their health in their team colour, with the name on it; yours has a bright frame
- Fixed: text could turn into black blocks (and eat memory) when new text sizes appeared, e.g. hovering a menu button

## v0.1.1

- First downloadable build, for Windows and Linux
- Elimination styles: Rounds, Last Man Standing and Attrition
- Every limit can be turned off, even all of them for an endless match
- 3 second spawn protection with a turquoise glow that blinks before it ends
- Blood only drips on the death screen

## v0.1.0

- 3 maps: Foundry Ascent, Frostline Relay and Redstone Arches
- Deathmatch and Elimination, free-for-all or 2 to 4 teams with pickable colours
- 10 weapons, 4 powerups, thrusters, weapon and shield stations
- Bots with three difficulties, set per team
- 3 second countdown, 5 second respawn, blood that builds up as you get hurt
