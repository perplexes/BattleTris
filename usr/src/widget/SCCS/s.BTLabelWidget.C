h25040
s 00044/00037/00027
d D 1.2 01/10/21 19:25:15 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:14 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTLabelWidget.C
c Name history : 1 0 src/widget/BTLabelWidget.C
e
s 00064/00000/00000
d D 1.1 01/10/20 13:35:13 bmc 1 0
c date and time created 01/10/20 13:35:13 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTLabelWidget.C                                     */
/*    ASGN: Final                                               */
/*    DATE: Thu Apr 21 14:07:37 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTLabelWidget.H"

D 3
BTLabelWidget::BTLabelWidget(BTWidget *parent, char *const name, char *const title)
E 3
I 3
BTLabelWidget::BTLabelWidget(BTWidget *parent, const char *name,
    const char *title)
E 3
: BTWidget(parent)
{
D 3
  XmString str = XmStringCreateLtoR(title, XmFONTLIST_DEFAULT_TAG);
  me_  = XtVaCreateWidget(name, xmLabelWidgetClass, parent ? parent->getWidget() : 0,
                          XmNlabelString, str, NULL);
  XmStringFree(str);

  setupStructs();
E 3
I 3
	XmString str =
	    XmStringCreateLtoR((char *)title, XmFONTLIST_DEFAULT_TAG);
	me_  = XtVaCreateWidget(name, xmLabelWidgetClass,
	    parent ? parent->getWidget() : 0, XmNlabelString, str, NULL);
	XmStringFree(str);
	setupStructs();
E 3
}

D 3
BTLabelWidget::BTLabelWidget(BTWidget *parent, char *const name, char *const title,
                             Position x1, Position y1, Position x2, Position y2)
E 3
I 3
BTLabelWidget::BTLabelWidget(BTWidget *parent, const char *name,
   const char *title, Position x1, Position y1, Position x2, Position y2)
E 3
: BTWidget(parent)
{
D 3
  XmString str = XmStringCreateLtoR(title, XmFONTLIST_DEFAULT_TAG);
  me_  =
    XtVaCreateWidget(name, xmLabelWidgetClass, parent ? parent->getWidget() : 0,
		     XmNlabelString, str,
                     XmNtopAttachment, XmATTACH_POSITION, XmNtopPosition, y1,
                     XmNleftAttachment, XmATTACH_POSITION, XmNleftPosition, x1,
                     XmNwidth, x2 - x1 + 1, XmNheight, y2 - y1 + 1,
                     XmNresizePolicy, XmRESIZE_NONE, NULL);
  XmStringFree(str);
  setupStructs();
E 3
I 3
	XmString str =
	    XmStringCreateLtoR((char *)title, XmFONTLIST_DEFAULT_TAG);
	me_  = XtVaCreateWidget(name, xmLabelWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNlabelString, str,
	    XmNtopAttachment, XmATTACH_POSITION, XmNtopPosition, y1,
	    XmNleftAttachment, XmATTACH_POSITION, XmNleftPosition, x1,
	    XmNwidth, x2 - x1 + 1, XmNheight, y2 - y1 + 1,
	    XmNresizePolicy, XmRESIZE_NONE, NULL);
	XmStringFree(str);
	setupStructs();
E 3
}

D 3
BTLabelWidget::BTLabelWidget(BTWidget *parent, char *const name, char *const title,
                             Dimension width, Dimension height)
E 3
I 3
BTLabelWidget::BTLabelWidget(BTWidget *parent, const char *name,
    const char *title, Dimension width, Dimension height)
E 3
: BTWidget(parent)
{
D 3
  XmString str = XmStringCreateLtoR(title, XmFONTLIST_DEFAULT_TAG);
  me_ =
    XtVaCreateWidget(name, xmLabelWidgetClass, parent ? parent->getWidget() : 0,
		     XmNlabelString, str,
                     XmNwidth, width, XmNheight, height,
                     XmNresizePolicy, XmRESIZE_NONE, NULL);
  XmStringFree(str);
  setupStructs();
E 3
I 3
	XmString str =
	    XmStringCreateLtoR((char *)title, XmFONTLIST_DEFAULT_TAG);
	me_ = XtVaCreateWidget(name, xmLabelWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNlabelString, str,
	    XmNwidth, width, XmNheight, height,
	    XmNresizePolicy, XmRESIZE_NONE,
	    NULL);
	XmStringFree(str);
	setupStructs();
E 3
}

D 3
void BTLabelWidget::setLabel(char * new_label)
E 3
I 3
void
BTLabelWidget::setLabel(const char *new_label)
E 3
{
D 3
  XmString str = XmStringCreateLtoR(new_label, XmFONTLIST_DEFAULT_TAG);
  XtVaSetValues(me_, XmNlabelString, str, NULL);
  XmStringFree(str);
E 3
I 3
	XmString str =
	    XmStringCreateLtoR((char *)new_label, XmFONTLIST_DEFAULT_TAG);
	XtVaSetValues(me_, XmNlabelString, (char *)str, NULL);
	XmStringFree(str);
E 3
}

D 3
void BTLabelWidget::setupStructs() {
  input_struct_ = 0;
  XtAddEventHandler(me_, ButtonPressMask, FALSE, 
                    input_CB, this);
E 3
I 3
void
BTLabelWidget::setupStructs()
{
	input_struct_ = 0;
	XtAddEventHandler(me_, ButtonPressMask, FALSE, input_CB, this);
E 3
}
E 1
