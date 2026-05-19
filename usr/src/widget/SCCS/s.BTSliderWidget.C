h37418
s 00000/00000/00000
d R 1.2 01/10/23 00:05:30 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 1 0 usr/src/widget/BTSliderWidget.C
e
s 00035/00000/00000
d D 1.1 01/10/23 00:05:29 bmc 1 0
c 1000017 Ernie needs levels other than "Hard" and "Impossible"
e
u
U
f e 0
t
T
I 1
#include "BTConfig.H"
#include "BTSliderWidget.H"
#include <Xm/Scale.h>

BTSliderWidget::BTSliderWidget(BTWidget *parent, const char *name, int nvalues)
: BTWidget(parent), value_(0), slider_cb_(NULL)
{
	me_  = XtVaCreateWidget(name, xmScaleWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNorientation, XmHORIZONTAL,
	    XmNmaximum, nvalues - 1,
	    XmNdecimalPoints, 0,
	    XmNshowValue, False,
	    XmNshowArrows, XmNONE,
	    XmNsliderMark, XmNONE,
	    NULL);

	XtAddCallback(me_, XmNvalueChangedCallback, change_CB, this);
	XtAddCallback(me_, XmNdragCallback, change_CB, this);
}

void
BTSliderWidget::change_CB(Widget widget, XtPointer data, XtPointer cbdata)
{
	BTSliderWidget *t = (BTSliderWidget *)data;
	XmScaleCallbackStruct *state = (XmScaleCallbackStruct *)cbdata;
	t->value_ = state->value;
	t->callback(t->slider_cb_);
}

void
BTSliderWidget::addChangeCallback(void (*cb)(BTWidget *, void *), void *data)
{
	addCallback(slider_cb_, cb, data);
}
E 1
