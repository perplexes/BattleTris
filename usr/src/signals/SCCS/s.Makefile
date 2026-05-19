h06067
s 00000/00000/00000
d R 1.2 01/10/20 13:34:59 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/signals/Makefile
c Name history : 1 0 src/signals/Makefile
e
s 00042/00000/00000
d D 1.1 01/10/20 13:34:58 bmc 1 0
c date and time created 01/10/20 13:34:58 by bmc
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
#    DESCR: Makefile for BattleTris signals library
# MODIFIED: 26Oct95
#

include ../Makeinclude

LIBNAME	= BTSignals
LIBRARY	= lib$(LIBNAME).a

LIBOBJ	= SigReceiver.o
LIBINC	= SigHandler.H SigReceiver.H

DSTLIB	= $(DSTLIBDIR)/$(LIBRARY)
DSTINC	= $(DSTINCDIR)/SigHandler.H $(DSTINCDIR)/SigReceiver.H

IFLAGS	= $(BT_IFLAGS)
LDFLAGS	= $(BT_LDFLAGS)
CXXFLAGS= $(BT_CXXFLAGS)

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
