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

$(LIBRARY): $(LIBOBJ)
	$(AR) $@ $?
	$(RANLIB) $@

.C.o:
	$(CXX) $(CXXFLAGS) $(IFLAGS) -c $<

clean:
	$(RM) $(LIBRARY) $(LIBOBJ) core

install: $(DSTLIB) $(DSTINC)
