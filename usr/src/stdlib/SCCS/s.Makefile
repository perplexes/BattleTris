h28043
s 00000/00000/00000
d R 1.2 01/10/20 13:35:03 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/stdlib/Makefile
c Name history : 1 0 src/stdlib/Makefile
e
s 00047/00000/00000
d D 1.1 01/10/20 13:35:02 bmc 1 0
c date and time created 01/10/20 13:35:02 by bmc
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
#     DATE: 11Aug95
#    DESCR: Makefile for BattleTris stdlib
# MODIFIED: 26Oct95
#

include ../Makeinclude

LIBNAME	= BTStdLib
LIBRARY	= lib$(LIBNAME).a

LIBOBJ	= AbsList.o AbsListElement.o AbsListIter.o \
	List.o ListElement.o ListIter.o Block.o
LIBINC	= AbsList.H AbsListElement.H AbsListIter.H \
	List.H ListElement.H ListIter.H RWListIter.H Block.H

DSTLIB	= $(DSTLIBDIR)/$(LIBRARY)
DSTINC	= $(DSTINCDIR)/AbsList.H $(DSTINCDIR)/AbsListElement.H \
	$(DSTINCDIR)/AbsListIter.H $(DSTINCDIR)/List.H \
	$(DSTINCDIR)/ListElement.H $(DSTINCDIR)/ListIter.H \
	$(DSTINCDIR)/RWListIter.H $(DSTINCDIR)/Block.H

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
