TODO:
 - fix strange ruby sed thingy, instead install ruby of correct versino from Gemfile.
 - cleanup pipeline.rs from language-specific stuff.
 - fix health endpoint test - we should pass healthcheck, 404 is not good enough. health endpoint should be guessed or detected correctly (although it can be / if no other page is available)
 - for some reason cache is not fully used
 - strange python venv handling with sed