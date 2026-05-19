h59795
s 00000/00000/00000
d R 1.2 01/10/23 00:05:30 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 1 0 usr/src/widget/BTSliderWidget.H
e
s 00020/00000/00000
d D 1.1 01/10/23 00:05:29 bmc 1 0
c 1000017 Ernie needs levels other than "Hard" and "Impossible"
e
u
U
f e 0
t
T
I 1
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
E 1
