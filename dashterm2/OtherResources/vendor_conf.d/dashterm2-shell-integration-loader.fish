#!/bin/fish

# To use fish's autoloading feature, DashTerm2 prepends the vendored integration script directory to XDG_DATA_DIRS.
# The original paths needs to be restored here to not affect other programs.
# In particular, if the original XDG_DATA_DIRS does not exist, it needs to be removed.
if set -q IT2_FISH_XDG_DATA_DIRS
    if set -q XDG_DATA_DIRS
        # At this point XDG_DATA_DIRS will be it2-si-dir:real-xdg-data-dirs
        set --global --export --path XDG_DATA_DIRS "$XDG_DATA_DIRS"
        if set --local index (contains --index "$IT2_FISH_XDG_DATA_DIRS" $XDG_DATA_DIRS)
            set --erase --global XDG_DATA_DIRS[$index]
            test -n "$XDG_DATA_DIRS" || set --erase --global XDG_DATA_DIRS
        end
        if set -q XDG_DATA_DIRS
            set --global --export --unpath XDG_DATA_DIRS "$XDG_DATA_DIRS"
        end
    end
end

status is-interactive || exit 0
not functions -q __it2_schedule || exit 0
# Check fish version 3.3.0+ efficiently and fallback to check the minimum working version 3.2.0, exit on outdated versions.
set -q fish_killring || set -q status_generation || string match -qnv "3.1.*" "$version"
or echo Warning: Update fish to version 3.3.0+ to enable automatic DashTerm2 shell integration loading. && exit 0 || exit 0

function __it2_schedule --on-event fish_prompt -d "Setup DashTerm2 integration after other scripts have run, we hope"
    functions --erase __it2_schedule
    if set -q IT2_FISH_XDG_DATA_DIRS
        set -l integration_dir "$IT2_FISH_XDG_DATA_DIRS"
        set -l integration_file "$integration_dir/dashterm2_shell_integration.fish"
        if test -f "$integration_file"
            source "$integration_file"
        else
            echo "Warning: DashTerm2 integration file $integration_file missing" 1>&2
        end
    end
    set --erase IT2_FISH_XDG_DATA_DIRS
end
