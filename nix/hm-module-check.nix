# Verifies the Home Manager module without importing home-manager itself: the
# module is evaluated against minimal stubs of the options it consumes, then the
# generated configuration is asserted.
{ pkgs, lib }:

let
  self = {
    packages.${pkgs.stdenv.hostPlatform.system}.mimo_manager = pkgs.hello;
  };
  module = import ./hm-module.nix self;

  stubs = {
    options.home.packages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default = [ ];
    };
    options.xdg.configFile = lib.mkOption {
      type = lib.types.attrsOf (
        lib.types.submodule {
          options.source = lib.mkOption { type = lib.types.anything; };
        }
      );
      default = { };
    };
  };

  evaluate =
    settings:
    lib.evalModules {
      modules = [
        { _module.args.pkgs = pkgs; }
        module
        stubs
        {
          programs.mimo_manager.enable = true;
          programs.mimo_manager.package = pkgs.hello;
          programs.mimo_manager.settings = settings;
        }
      ];
    };

  populated = evaluate {
    theme = "MELANGE DARK";
    confirm_destructive = false;
    modeline.status = false;
  };
  empty = evaluate { };

  writesFile = populated.config.xdg.configFile ? "mimo_manager/config.toml";
  emptyWritesFile = empty.config.xdg.configFile ? "mimo_manager/config.toml";
  generated = populated.config.xdg.configFile."mimo_manager/config.toml".source;
in
assert writesFile;
assert !emptyWritesFile;
pkgs.runCommand "mimo_manager-hm-module-check" { } ''
  grep -qF 'confirm_destructive = false' ${generated}
  grep -qF 'theme = "MELANGE DARK"' ${generated}
  grep -qF '[modeline]' ${generated}
  grep -qF 'status = false' ${generated}
  touch $out
''
