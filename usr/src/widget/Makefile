#
#     NAME: Makefile
#   AUTHOR: Michael Shapiro
#     DATE: 4May94
#    DESCR: Makefile for BattleTris Motif Widget library
# MODIFIED: 31Oct95
#

include ../Makeinclude

LIBNAME	= BTWidget
LIBRARY	= lib$(LIBNAME).a

LIBOBJ	= \
	BTWidget.o \
	BTDrawingAreaWidget.o \
	BTFormWidget.o \
	BTLabelWidget.o \
	BTMessageDlog.o \
	BTPushButtonWidget.o \
	BTRowColumnWidget.o \
	BTSliderWidget.o \
	BTScrolledListWidget.o \
	BTScrolledTextWidget.o \
	BTTextWidget.o \
	BTPixmap.o \
	BTFrameWidget.o \
	BTDisplay.o \
	BTXDisplay.o \
	BTXPalette.o \
	BTXmUtils.o

LIBINC	= \
	BTDrawingAreaWidget.H \
	BTFormWidget.H \
	BTLabelWidget.H \
	BTMessageDlog.H \
	BTPushButtonWidget.H \
	BTRowColumnWidget.H \
	BTSliderWidget.H \
	BTScrolledListWidget.H \
	BTScrolledTextWidget.H \
	BTStatusDlog.H \
	BTTextWidget.H \
	BTFrameWidget.H \
	BTCheckBoxWidget.H \
	BTWidget.H \
	BTPixmap.H \
	BTDisplay.H \
	BTXDisplay.H \
	BTPalette.H \
	BTColor.H \
	BTXColor.H \
	BTXPalette.H

DSTLIB	= $(DSTLIBDIR)/$(LIBRARY)
DSTINC	= \
	$(DSTINCDIR)/BTDrawingAreaWidget.H \
	$(DSTINCDIR)/BTFormWidget.H \
	$(DSTINCDIR)/BTLabelWidget.H \
	$(DSTINCDIR)/BTMessageDlog.H \
	$(DSTINCDIR)/BTPushButtonWidget.H \
	$(DSTINCDIR)/BTRowColumnWidget.H \
	$(DSTINCDIR)/BTScrolledListWidget.H \
	$(DSTINCDIR)/BTScrolledTextWidget.H \
	$(DSTINCDIR)/BTSliderWidget.H \
	$(DSTINCDIR)/BTStatusDlog.H \
	$(DSTINCDIR)/BTTextWidget.H \
	$(DSTINCDIR)/BTFrameWidget.H \
	$(DSTINCDIR)/BTCheckBoxWidget.H \
	$(DSTINCDIR)/BTWidget.H \
	$(DSTINCDIR)/BTDisplay.H \
	$(DSTINCDIR)/BTXDisplay.H \
	$(DSTINCDIR)/BTPalette.H \
	$(DSTINCDIR)/BTColor.H \
	$(DSTINCDIR)/BTXColor.H \
	$(DSTINCDIR)/BTXPalette.H \
	$(DSTINCDIR)/BTPixmap.H

IFLAGS	= $(BT_IFLAGS)
LDFLAGS	= $(BT_LDFLAGS) $(MOTIF_LIBS) $(X11_LIBS)
CXXFLAGS= $(BT_CXXFLAGS) $(MOTIF_CFLAGS) $(X11_CFLAGS)

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
