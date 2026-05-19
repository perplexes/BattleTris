#ifndef BTSLIDERWIDGET_H
#define BTSLIDERWIDGET_H

#include "BTConfig.H"
#include "BTWidget.H"

class BTSliderWidget : public BTWidget {
protected:
	static void change_CB(Widget widget, XtPointer data, XtPointer cdata);
	ActivateCBStruct *slider_cb_;

public:
	int value_;

	BTSliderWidget(BTWidget *parent, const char *name, int nvalues);
	virtual void addChangeCallback(void (*cb)(BTWidget *, void *), void *);
	~BTSliderWidget() {}
};

#endif
