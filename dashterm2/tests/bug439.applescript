tell application "DashTerm2"
	activate
	tell the first terminal
		launch session "Default Session"
		tell the last session
			write text "top"
		end tell
	end tell
end tell
