# This program is free software; you can redistribute it and/or
# modify it under the terms of the GNU General Public License
# as published by the Free Software Foundation; either version 2
# of the License, or (at your option) any later version.
# 
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
# 
# You should have received a copy of the GNU General Public License
# along with this program; if not, write to the Free Software
# Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301, USA.

# Note that tcsh doesn't allow the prompt to end in an escape code so the terminal space here is required. DashTerm2 ignores spaces after this code.
# This is the second version of this script. It rejects "screen" terminals and uses aliases to make the code readable.

# Prevent the script from running twice.
if ( ! ($?dashterm2_shell_integration_installed)) then
  # Make sure this is an interactive shell.
  if ($?prompt) then

    # Define aliases for the start and end of OSC escape codes used by shell integration.
    if ( ! ($?ITERM_ENABLE_SHELL_INTEGRATION_WITH_TMUX)) then
      setenv ITERM_ENABLE_SHELL_INTEGRATION_WITH_TMUX ""
    endif

    if ( x"$ITERM_ENABLE_SHELL_INTEGRATION_WITH_TMUX""$TERM" != xscreen && x"$ITERM_ENABLE_SHELL_INTEGRATION_WITH_TMUX""$TERM" != xtmux-256color && x"$TERM" != xlinux && x"$TERM" != xdumb ) then


      set dashterm2_shell_integration_installed="yes"

      alias _dashterm2_start 'printf "\033]"'
      alias _dashterm2_end 'printf "\007"'
      alias _dashterm2_end_prompt 'printf "\007"'

      # Define aliases for printing the current hostname
      # If hostname -f is slow to run on your system, set dashterm2_hostname before sourcing this script.
      if ( ! ($?dashterm2_hostname)) then
          # hostname is fast on macOS so don't cache it. This lets us have an up to date value if it
          # changes because you connect to a VPN, for example.
          if ( `uname` != Darwin ) then
              set dashterm2_hostname=`hostname -f |& cat || false`
              # some flavors of BSD (i.e. NetBSD and OpenBSD) don't have the -f option
              if ( $status != 0 ) then
                  set dashterm2_hostname=`hostname`
              endif
          endif
      endif
      if ( ! ($?dashterm2_hostname)) then
          alias _dashterm2_print_remote_host 'printf "1337;RemoteHost=%s@%s" "$USER" `/bin/sh -c "hostname -f 2>/dev/null || hostname"`'
      else
          alias _dashterm2_print_remote_host 'printf "1337;RemoteHost=%s@%s" "$USER" "$dashterm2_hostname"'
      endif
      alias _dashterm2_remote_host "(_dashterm2_start; _dashterm2_print_remote_host; _dashterm2_end)"

      # Define aliases for printing the current directory
      alias _dashterm2_print_current_dir 'printf "1337;CurrentDir=$PWD"'
      alias _dashterm2_current_dir "(_dashterm2_start; _dashterm2_print_current_dir; _dashterm2_end)"

      # Define aliases for printing the shell integration version this script is written against
      alias _dashterm2_print_shell_integration_version 'printf "1337;ShellIntegrationVersion=7;shell=tcsh"'
      alias _dashterm2_shell_integration_version "(_dashterm2_start; _dashterm2_print_shell_integration_version; _dashterm2_end)"

      # Define aliases for defining the boundary between a command prompt and the
      # output of a command started from that prompt.
      if (! $?TERM_PROGRAM) then
          alias _dashterm2_print_between_prompt_and_exec 'printf "133;C;"'
      else
        if ( x"$TERM_PROGRAM" != x"DashTerm.app" ) then
          alias _dashterm2_print_between_prompt_and_exec 'printf "133;C;"'
        else
          alias _dashterm2_print_between_prompt_and_exec 'printf "133;C;\r"'
        endif
      endif

      alias _dashterm2_between_prompt_and_exec "(_dashterm2_start; _dashterm2_print_between_prompt_and_exec; _dashterm2_end)"

      # Define aliases for defining the start of a command prompt.
      alias _dashterm2_print_before_prompt 'printf "133;A"'
      alias _dashterm2_before_prompt "(_dashterm2_start; _dashterm2_print_before_prompt; _dashterm2_end_prompt)"

      # Define aliases for defining the end of a command prompt.
      alias _dashterm2_print_after_prompt 'printf "133;B"'
      alias _dashterm2_after_prompt "(_dashterm2_start; _dashterm2_print_after_prompt; _dashterm2_end_prompt)"
       
      # Define aliases for printing the status of the last command.
      alias _dashterm2_last_status 'printf "\033]133;D;$?\007"'

      # Usage: dashterm2_set_user_var key `printf "%s" value | base64`
      alias dashterm2_set_user_var 'printf "\033]1337;SetUserVar=%s=%s\007"'

      # User may override this to set user-defined vars. It should look like this, because your shell is terrible for scripting:
      # alias _dashterm2_user_defined_vars (dashterm2_set_user_var key1 `printf "%s" value1 | base64`; dashterm2_set_user_var key2 `printf "%s" value2 | base64`; ...)
      (which _dashterm2_user_defined_vars >& /dev/null) || alias _dashterm2_user_defined_vars ''

      # Combines all status update aliases
      alias _dashterm2_update_current_state '_dashterm2_remote_host; _dashterm2_current_dir; _dashterm2_user_defined_vars'

      # This is necessary so the first command line will have a hostname and current directory.
      _dashterm2_update_current_state
      _dashterm2_shell_integration_version

      # Define precmd, which runs just before the prompt is printed. This could go
      # in $prompt but this keeps things a little simpler in here.
      # No parens or dashterm2_start call is allowed prior to evaluating the last status.
      alias precmd '_dashterm2_last_status; _dashterm2_update_current_state'

      # Define postcmd, which runs just before a command is executed.
      alias postcmd '(_dashterm2_between_prompt_and_exec)'

      # Quotes are ignored inside backticks, so use noglob to prevent bug 3393.
      set noglob

      # Remove the terminal space from the prompt to work around a tcsh bug.
      # Set the echo_style so Centos (and perhaps others) will handle multi-
      # line prompts correctly.
      set _dashterm2_saved_echo_style=$echo_style
      set echo_style=bsd
      set _dashterm2_truncated_prompt=`echo "$prompt" | sed -e 's/ $//'`
      set echo_style=$_dashterm2_saved_echo_style
      unset _dashterm2_saved_echo_style

      # Wrap the prompt in FinalTerm escape codes and re-add a terminal space.
      set prompt="%{"`_dashterm2_before_prompt`"%}$_dashterm2_truncated_prompt%{"`_dashterm2_after_prompt`"%} "

      # Turn globbing back on.
      unset noglob
    endif
  endif
endif
