{
  description = "Bims — a 2D top-down game in Rust and Bevy";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { nixpkgs, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      eachSystem = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});

      # The classes a `combat_droids_<class>` command is spelled with —
      # features 79 and 83, `world::Class::ALL` bar the classless one, in the words
      # `names::CLASS_NAMES` gives them. A class added to the game is a
      # name added here; `bims list` is the build's own answer, and
      # `./check` opens each of these, so a drift is a red step rather
      # than a quiet absence.
      combatClasses = [
        "engineer"
        "soldier"
        "medic"
        "tank"
        "commander"
      ];

      # What a class's command says it opens, in one place, since the
      # package and the app each say it. "an engineer", "a tank". The
      # machines' fight is the only one since every enemy is a machine
      # (feature 102): the `combat_<class>` commands were the human
      # garrison's, and went with it.
      droidsAbout =
        class:
        let
          a = if builtins.elem (builtins.substring 0 1 class) [ "a" "e" "i" "o" "u" ] then "an" else "a";
        in
        "The machines' fight with the crew member you steer ${a} ${class}";

      # Build inputs only. `target/` is an output, so leaving it out keeps
      # the hash from churning on rebuilds.
      src = nixpkgs.lib.cleanSourceWith {
        src = ./.;
        name = "bims-source";
        filter =
          path: type:
          let
            base = baseNameOf (toString path);
          in
          !(type == "directory" && (base == "target" || base == "result"));
      };

      # Everything the flake exposes, built once per system.
      bimsFor =
        pkgs:
        let
          # What winit and wgpu open at runtime: the window system and the
          # GPU loader. None of it is linked at build time, so the binary is
          # wrapped to find them rather than built against them.
          runtimeLibs = with pkgs; [
            vulkan-loader
            libxkbcommon
            wayland
            libx11
            libxcursor
            libxi
            libxrandr
          ];

          bims = pkgs.rustPlatform.buildRustPackage {
            pname = "bims";
            version = "0.1.0";
            inherit src;

            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = [
              pkgs.makeWrapper
              pkgs.pkg-config
            ];
            # ALSA is linked, not opened at run time: cpal, under Bevy's
            # audio, finds it through pkg-config. The sounds themselves are
            # bytes in the binary (`crates/app/sounds/`), so nothing else
            # is installed.
            buildInputs = [ pkgs.alsa-lib ];

            # The whole workspace is tested — the rules crates carry the
            # tests that pin the world's arithmetic, and the app the ones
            # that pin its tables against them — but only the app is built
            # for the result, since it is the one binary.
            cargoBuildFlags = [
              "-p"
              "app"
            ];
            cargoTestFlags = [ "--workspace" ];

            postInstall = ''
              wrapProgram "$out/bin/bims" \
                --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeLibs}
            '';

            meta = {
              description = "A 2D top-down game: a crew, a ship and a system to fly it round";
              mainProgram = "bims";
              platforms = systems;
            };
          };

          # The relay, for the server box (feature 59): `crates/server`,
          # the one other binary. Only `wire` and tokio behind it — no
          # Bevy, no ALSA, no GPU stack — so the box that has none of those
          # builds it, and a fix to the game never means rebuilding it.
          # The workspace's own lock, so the two are pinned together.
          bims-server = pkgs.rustPlatform.buildRustPackage {
            pname = "bims-server";
            version = "0.1.0";
            inherit src;

            cargoLock.lockFile = ./Cargo.lock;

            cargoBuildFlags = [
              "-p"
              "server"
            ];
            # The relay's own tests and the wire's; the socket test starts
            # the binary it just built on a free port.
            cargoTestFlags = [
              "-p"
              "server"
              "-p"
              "wire"
            ];

            meta = {
              description = "The Bims relay: rooms by code, and bytes passed between the players in one";
              mainProgram = "bims-server";
              platforms = systems;
            };
          };


          # One per thing to run. Each is a name rather than a flag on one
          # app, so `nix run .#droids` reads as what it is.
          runFor =
            {
              name,
              what,
              about,
              # Lines run before the exec: the station builder points its
              # sketches at the working directory, since a store path is
              # nowhere to save.
              before ? "",
            }:
            pkgs.writeShellApplication {
              inherit name;
              text = ''
                ${before}
                exec ${nixpkgs.lib.getExe bims} ${what} "$@"
              '';
              meta.description = about;
            };
        in
        {
          inherit bims bims-server;
          bims-game = runFor {
            name = "bims-game";
            what = "game";
            about = "Play Bims — menu, lobby, world, ship, then the game";
          };
          bims-simulation = runFor {
            name = "bims-simulation";
            what = "simulation";
            about = "Straight into the game world on the playtest ship";
          };
          bims-design = runFor {
            name = "bims-design";
            what = "design";
            about = "Straight into the yard, the playtest ship given, docked where the simulation docks";
          };
          bims-test = runFor {
            name = "bims-test";
            what = "test";
            about = "Docked at a random station in a random galaxy, on the playtest ship";
          };
          bims-test-planet = runFor {
            name = "bims-test-planet";
            what = "test_planet";
            about = "Set down on a planet in a random galaxy, on the playtest ship";
          };
          bims-stationbuilder = runFor {
            name = "bims-stationbuilder";
            what = "stationbuilder";
            before = ''export BIMS_STATIONS_DIR="''${BIMS_STATIONS_DIR:-$PWD/stations}"'';
            about = "A grid to sketch a station's rough shape on, saved as text for a plan to be written from";
          };
          bims-tier2-test = runFor {
            name = "bims-tier2-test";
            what = "tier2_test";
            about = "The fight with everybody's guns and armour at tier two, and the machines at it too";
          };
          bims-tier3-test = runFor {
            name = "bims-tier3-test";
            what = "tier3_test";
            about = "The fight with everybody's guns and armour at tier three, and the machines at it too";
          };
          bims-droids = runFor {
            name = "bims-droids";
            what = "droids";
            about = "The fight: the combat ship's crew at an arena the machines hold, a wave of droids about it";
          };
          bims-droids-planet = runFor {
            name = "bims-droids-planet";
            what = "droids_planet";
            about = "A town on a planet held by the machines, the ship set down at its pad";
          };
          bims-crisis = runFor {
            name = "bims-crisis";
            what = "crisis";
            about = "The simulation a day before the crisis first spreads, its origin two hyperlane hops off";
          };
          bims-jammer = runFor {
            name = "bims-jammer";
            what = "jammer";
            about = "The crew in an infested system two hops from the origin: the jammer standing, a wave aboard, the lanes inward shut";
          };
          bims-defense = runFor {
            name = "bims-defense";
            what = "defense";
            about = "A town with the machines one hop off: the crew set down at its pad, and a wave landing outside a gate a minute later";
          };
        }
        # The machines' fight, one build a class, so a class is looked at
        # in the fight it is for without a `BIMS_CLASS` in front of the
        # command (feature 79). The list is written here rather than read
        # off `Class::ALL`, since nix cannot ask the binary; `bims list` is
        # what always agrees with the build.
        // nixpkgs.lib.listToAttrs (
          map
            (class: {
              name = "bims-droids-${class}";
              value = runFor {
                name = "bims-droids-${class}";
                what = "combat_droids_${class}";
                about = droidsAbout class;
              };
            })
            combatClasses
        );
    in
    {
      packages = eachSystem (
        pkgs:
        let
          built = bimsFor pkgs;
        in
        {
          inherit (built)
            bims
            bims-game
            bims-simulation
            bims-design
            bims-test
            bims-test-planet
            bims-droids
            bims-tier2-test
            bims-tier3-test
            bims-crisis
            bims-jammer
            bims-stationbuilder
            bims-server
            ;
          default = built.bims;
        }
        # One package a class's fight against the machines as well
        # (feature 79).
        // nixpkgs.lib.getAttrs (map (class: "bims-droids-${class}") combatClasses) built
      );

      # One app per thing you can run. `nix run .#game` is the whole game in
      # the order a player meets it and is the default; `.#simulation` skips
      # to the world on a prebuilt ship; `.#design` skips to the yard with
      # that ship given; `.#test` is the simulation somewhere else each
      # time; `.#test_planet` is that set down
      # on a planet; `.#droids` is the fight, at an arena the machines
      # hold — every enemy is one since feature 102, so the human
      # garrison's `.#combat` and `.#combat_<class>` and the `.#raid` are
      # gone — `.#combat_droids_engineer` … `.#combat_droids_commander`
      # that same wave with the crew member you steer that class, and
      # `.#droids_planet` a town on a planet held by them;
      # `.#tier2_test` and `.#tier3_test` are the fight with every gun and
      # every piece of armour at that tier, both sides;
      # `.#crisis` is the simulation a day before the crisis first spreads,
      # its origin two hyperlane hops off, so the next stars turn red
      # while you watch;
      # `.#jammer` is one step on from that: the crew in an infested system
      # two hops from the origin, its jammer standing and a wave aboard, so
      # the lanes inward are shut and the chart says so;
      # saves beside the working tree rather than inside the store;
      # `.#server` is the relay the game finds its crew through, run on
      # the server box rather than a desk.
      # saves beside the working tree rather than inside the store.
      apps = eachSystem (
        pkgs:
        let
          built = bimsFor pkgs;
          app = drv: about: {
            type = "app";
            program = nixpkgs.lib.getExe drv;
            meta.description = about;
          };
        in
        rec {
          game = app built.bims-game "Play Bims — menu, lobby, world, then the run on the default ship";
          simulation = app built.bims-simulation "Straight into the game world on the playtest ship";
          design = app built.bims-design "Straight into the yard, the playtest ship given, docked where the simulation docks";
          test = app built.bims-test "Docked at a random station in a random galaxy, on the playtest ship";
          test_planet = app built.bims-test-planet "Set down on a planet in a random galaxy, on the playtest ship";
          tier2_test = app built.bims-tier2-test "The fight with everybody's guns and armour at tier two, and the machines at it too";
          tier3_test = app built.bims-tier3-test "The fight with everybody's guns and armour at tier three, and the machines at it too";
          droids = app built.bims-droids "The fight: the combat ship's crew at an arena the machines hold, a wave of droids about it";
          droids_planet = app built.bims-droids-planet "A town on a planet held by the machines, the ship set down at its pad";
          crisis = app built.bims-crisis "The simulation a day before the crisis first spreads, its origin two hyperlane hops off";
          jammer = app built.bims-jammer "The crew in an infested system two hops from the origin: the jammer standing, a wave aboard, the lanes inward shut";
          defense = app built.bims-defense "A town with the machines one hop off: the crew set down at its pad, and a wave landing outside a gate a minute later";
          stationbuilder = app built.bims-stationbuilder "A grid to sketch a station's rough shape on, saved as text for a plan to be written from";
          server = app built.bims-server "The relay: rooms by code, and bytes passed between the players in one — what runs at bims.buggly.de";
          default = game;
        }
        # `.#combat_droids_medic` and the rest: the machines' fight, the
        # crew member you steer that class.
        // nixpkgs.lib.listToAttrs (
          map
            (class: {
              name = "combat_droids_${class}";
              value = app built."bims-droids-${class}" (droidsAbout class);
            })
            combatClasses
        )
      );

      # Shared with the plain `nix-shell` entry point, so there is one list of
      # development tools rather than two that drift apart.
      devShells = eachSystem (pkgs: {
        default = import ./shell.nix { inherit pkgs; };
      });

      checks = eachSystem (
        pkgs:
        let
          built = bimsFor pkgs;
        in
        {
          # Builds the app and runs every crate's tests: buildRustPackage's
          # check phase is `cargo test`, with the flags above.
          build = built.bims;

          formatting =
            pkgs.runCommand "bims-check-formatting"
              {
                nativeBuildInputs = [
                  pkgs.cargo
                  pkgs.rustfmt
                ];
              }
              ''
                # rustfmt wants a writable tree even when only checking.
                cp -r ${src} source
                chmod -R u+w source
                cd source
                export CARGO_HOME="$NIX_BUILD_TOP/cargo"
                cargo fmt --check
                touch "$out"
              '';
        }
      );

      formatter = eachSystem (pkgs: pkgs.nixfmt-rfc-style);
    };
}
