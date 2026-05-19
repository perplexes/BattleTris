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
