h20559
s 00000/00000/00000
d R 1.2 01/10/20 13:35:42 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/acconfig.h
c Name history : 1 0 src/acconfig.h
e
s 00021/00000/00000
d D 1.1 01/10/20 13:35:41 bmc 1 0
c CodeManager Uniquification: src/acconfig.h
c date and time created 01/10/20 13:35:41 by bmc
e
u
U
f e 0
t
T
I 1
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
E 1
