self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.mimo_manager;
  tomlFormat = pkgs.formats.toml { };
in
{
  options.programs.mimo_manager = {
    enable = lib.mkEnableOption "mimo_manager, a keyboard-centric MIME type and default-application manager";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.mimo_manager;
      defaultText = lib.literalExpression "mimo_manager.packages.\${system}.mimo_manager";
      description = "The mimo_manager package to install.";
    };

    settings = lib.mkOption {
      type = tomlFormat.type;
      default = { };
      description = ''
        Configuration written to {file}`$XDG_CONFIG_HOME/mimo_manager/config.toml`.
        When empty, no file is written and the application uses its defaults.
      '';
      example = lib.literalExpression ''
        {
          theme = "MELANGE DARK";
          confirm_destructive = true;
          modeline.status = false;
        }
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];

    xdg.configFile."mimo_manager/config.toml" = lib.mkIf (cfg.settings != { }) {
      source = tomlFormat.generate "mimo_manager-config.toml" cfg.settings;
    };
  };
}
