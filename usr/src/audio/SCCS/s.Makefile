h15634
s 00002/00002/00040
d D 1.2 01/10/21 01:52:45 bmc 3 1
c 1000007 audio relies on broken, unshipped header files, libraries
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:06 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/audio/Makefile
c Name history : 1 0 src/audio/Makefile
e
s 00042/00000/00000
d D 1.1 01/10/20 13:35:05 bmc 1 0
c date and time created 01/10/20 13:35:05 by bmc
e
u
U
f e 0
t
T
I 1
#
#     NAME: Makefile
#   AUTHOR: Michael Shapiro
#     DATE: 13Oct94
#    DESCR: Makefile for BattleTris audio library
# MODIFIED: 26Oct95
#

include ../Makeinclude

LIBNAME	= BTAudio
LIBRARY	= lib$(LIBNAME).a

LIBOBJ	= DevAudio.o
LIBINC	= DevAudio.H

DSTLIB	= $(DSTLIBDIR)/$(LIBRARY)
DSTINC	= $(DSTINCDIR)/DevAudio.H

IFLAGS	= $(BT_IFLAGS)
D 3
LDFLAGS	= $(BT_LDFLAGS) $(AUDIO_LIBS)
CXXFLAGS= $(BT_CXXFLAGS) $(AUDIO_CFLAGS)
E 3
I 3
LDFLAGS	= $(BT_LDFLAGS)
CXXFLAGS= $(BT_CXXFLAGS)
E 3

all:	$(LIBRARY)

$(DSTLIB): $(LIBRARY)
	$(INSTALL) -m 0444 $(LIBRARY) $@

$(DSTINC): $$(@F)
	$(INSTALL) -m 0444 $(@F) $@

$(LIBRARY): $(LIBOBJ)
	$(AR) $@ $?
	$(RANLIB) $@

.C.o:
	$(CXX) $(CXXFLAGS) $(IFLAGS) -c $<

clean:
	$(RM) $(LIBRARY) $(LIBOBJ) core

install: $(DSTLIB) $(DSTINC)
E 1
