/*
 * Defined automatically if --with-sun-audio is passed to configure to
 * enable the Sun audio code, which needs the libraries normally found
 * in /usr/demo/SOUND on Sun machines, or if configure finds /usr/demo/SOUND
 * on its own.
 */
#undef HAVE_SUNAUDIO

/*
 * Define the type of file descriptor set arguments to select.  The configure
 * script will try to determine this and attempt to compile a test program to
 * verify that it works.
 */
#undef SELECTARGTYPE

/*
 * Define a value to return from our master signal handling function which is
 * appropriate for the type of RETSIGTYPE.  By default we define RETSIGVAL as
 * an empty expansion if RETSIGTYPE is void.
 */
#undef RETSIGVAL
